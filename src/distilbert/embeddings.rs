// Copyright 2019-present, the HuggingFace Inc. team, The Google AI Language Team and Facebook, Inc.
// Copyright 2019 Guillaume Becquin
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use tch::{nn, Tensor, Kind, Device};
use tch::nn::{ModuleT, embedding, EmbeddingConfig};
use crate::distilbert::distilbert::DistilBertConfig;
use crate::common::dropout::Dropout;
use tch::kind::Kind::Float;


fn create_sinusoidal_embeddings(config: &DistilBertConfig, device: Device) -> nn::Embedding {
    let mut sinusoidal_embedding: Vec<Tensor> = Vec::with_capacity(config.max_position_embeddings as usize);
    for pos in 0..config.max_position_embeddings {
        let mut temp_vec: Vec<f64> = Vec::with_capacity(config.dim as usize);
        for j in 0..config.dim {
            if j % 2 == 0 {
                temp_vec.push((pos as f64 / 10000f64.powf((2 * (j / 2)) as f64 / config.dim as f64)).sin());
            } else {
                temp_vec.push((pos as f64 / 10000f64.powf((2 * (j / 2)) as f64 / config.dim as f64)).cos());
            }
        }
        let temp_vec = Tensor::of_slice(&temp_vec);
        sinusoidal_embedding.push(temp_vec);
    }
    let sinusoidal_embedding = Tensor::stack(&sinusoidal_embedding, 0).to_kind(Float);

    let embedding_config = EmbeddingConfig { padding_idx: 0, ..Default::default() };
    let mut embeddings = embedding(&nn::VarStore::new(device).root(),
                                   config.max_position_embeddings,
                                   config.dim,
                                   embedding_config);

    embeddings.ws = sinusoidal_embedding;
    embeddings
}


#[derive(Debug)]
pub struct DistilBertEmbedding {
    word_embeddings: nn::Embedding,
    position_embeddings: nn::Embedding,
    layer_norm: nn::LayerNorm,
    dropout: Dropout,
}

impl DistilBertEmbedding {
    pub fn new(p: &nn::Path, config: &DistilBertConfig) -> DistilBertEmbedding {
        let embedding_config = EmbeddingConfig { padding_idx: 0, ..Default::default() };

        let word_embeddings: nn::Embedding = embedding(p / "word_embeddings",
                                                       config.vocab_size,
                                                       config.dim,
                                                       embedding_config);
        let position_embeddings: nn::Embedding = match config.sinusoidal_pos_embds {
            false => embedding(p / "position_embeddings",
                               config.max_position_embeddings,
                               config.dim,
                               embedding_config),

            true => create_sinusoidal_embeddings(&config, p.device())
        };
        let layer_norm_config = nn::LayerNormConfig { eps: 1e-12, ..Default::default() };
        let layer_norm: nn::LayerNorm = nn::layer_norm(p / "LayerNorm", vec![config.dim], layer_norm_config);
        let dropout: Dropout = Dropout::new(config.dropout);
        DistilBertEmbedding { word_embeddings, position_embeddings, layer_norm, dropout }
    }

    pub fn _get_word_embeddings(&self) -> &nn::Embedding {
        &self.word_embeddings
    }

    pub fn _set_word_embeddings(&mut self, new_embeddings: nn::Embedding) {
        self.word_embeddings = new_embeddings;
    }
}

impl ModuleT for DistilBertEmbedding {
    fn forward_t(&self, input: &Tensor, train: bool) -> Tensor {
        let seq_length = (&input).size().last().unwrap().to_owned();
        let position_ids = Tensor::arange(seq_length, (Kind::Int64, input.device()));
        let position_ids = position_ids.unsqueeze(0).expand_as(input);

        let word_embed = input.apply(&self.word_embeddings);
        let position_embed = position_ids.apply(&self.position_embeddings);

//        position_embed.get(0).get(0).print();
        let embeddings = word_embed + position_embed;
        let embeddings = embeddings.apply(&self.layer_norm).apply_t(&self.dropout, train);

        embeddings
    }
}