use async_trait::async_trait;
use psyche::{Impression, Memory, QdrantVectorPoint, Stimulus, find_vector_clusters};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockMemory(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Memory for MockMemory {
    async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(impression.summary.clone());
        Ok(())
    }
}

#[tokio::test]
async fn stores_impression() {
    let mem = MockMemory::default();
    <dyn Memory>::store_serializable(
        &mem,
        &Impression::new(vec![Stimulus::new(1)], "hello", None::<String>),
    )
    .await
    .unwrap();
    assert_eq!(mem.0.lock().unwrap().len(), 1);
}

#[test]
fn finds_threshold_connected_vector_clusters() {
    let points = vec![
        vector_point("a", [1.0, 0.0]),
        vector_point("b", [0.95, 0.05]),
        vector_point("c", [0.0, 1.0]),
        vector_point("d", [0.05, 0.95]),
        vector_point("e", [-1.0, 0.0]),
    ];

    let clusters = find_vector_clusters("memories", &points, 0.9, 2);

    assert_eq!(clusters.len(), 2);
    let mut member_sets = clusters
        .iter()
        .map(|cluster| {
            cluster
                .members
                .iter()
                .map(|member| member.point_id.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    member_sets.sort();
    assert_eq!(member_sets, vec![vec!["a", "b"], vec!["c", "d"]]);
}

fn vector_point<const N: usize>(point_id: &str, vector: [f32; N]) -> QdrantVectorPoint {
    QdrantVectorPoint {
        point_id: point_id.into(),
        vector: vector.to_vec(),
        payload: json!({}),
    }
}
