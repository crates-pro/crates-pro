use std::{
    collections::{HashSet, VecDeque},
    error::Error,
};

use model::general_model::Version;
use tugraph::{
    cursor::EdgeCursor, cursor::VertexCursor, db::Graph, field::FieldData, txn::TxnRead,
};

#[tugraph_plugin_util::tugraph_plugin]
fn compute_impact_scope(graph: &mut Graph, req: &str) -> Result<String, Box<dyn Error>> {
    // req stores the request string from the web
    // its format is "person_name,movie_title"
    // parse from req to get person_name and movie_title
    let dst_version = parse_req(req)?;
    let dst_tu_version = dst_version.name + "/" + &dst_version.version;

    // create read only transaction
    let ro_txn = graph.create_ro_txn()?;

    // require the start node
    let mut dst_version_iter = ro_txn.vertex_index_iter_ids_from(
        "version",
        "name_and_version",
        &FieldData::String(dst_tu_version.clone()),
        &FieldData::String(dst_tu_version.clone()),
    )?;
    let dst_id = dst_version_iter.next().ok_or(format!(
        "the dst node {:?} not found",
        dst_tu_version.clone()
    ))?;

    // BFS to search
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back((dst_id, dst_tu_version, 0_i64));

    while let Some((node_id, _, depth)) = queue.pop_front() {
        let mut node_cursor = ro_txn.vertex_cur()?;
        node_cursor.seek(node_id, false)?;
        assert!(node_cursor.is_valid());

        // Get all neighbor pointing to the current node
        let neighbors: Vec<_> = node_cursor
            .in_edge_cursor()?
            .into_edges()
            .filter(|(_, label, _)| label == "depends_on")
            .collect();

        for (id, _, _) in neighbors {
            let mut user_cur = ro_txn.vertex_cur()?;
            user_cur.seek(id.src, false)?;
            let nv = user_cur
                .field("name_and_version")
                .map(|name| match name {
                    FieldData::String(name) => name,
                    _ => panic!("name should be string"),
                })
                .unwrap();

            let in_neighbor = (id.src, nv, depth + 1);
            if visited.insert(in_neighbor.clone()) {
                queue.push_back(in_neighbor);
            }
        }
    }
    let res: Vec<(i64, String, i64)> = visited.into_iter().collect();
    let res = serde_json::to_string(&res).unwrap();
    Ok(res)
}

fn parse_req(req: &str) -> Result<Version, String> {
    let certain_version: Vec<String> = req.split(' ').map(|x| x.to_string()).collect();

    if certain_version.len() != 2 {
        return Err("Format Error! The argument should be passed as `random 0.8.4`".to_string());
    }

    let certain_version = Version {
        name: certain_version[0].to_string(),
        version: certain_version[1].to_string(),
    };

    Ok(certain_version)
}
