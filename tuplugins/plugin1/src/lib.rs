use std::{
    collections::{HashSet, VecDeque},
    error::Error,
};

use model::general_model::Version;
use tugraph::{
    cursor::EdgeCursor, cursor::VertexCursor, db::Graph, field::FieldData, txn::TxnRead,
};

#[tugraph_plugin_util::tugraph_plugin]
fn track_dependency(graph: &mut Graph, req: &str) -> Result<String, Box<dyn Error>> {
    // req stores the request string from the web
    // its format is "person_name,movie_title"
    // parse from req to get person_name and movie_title
    println!("Exec trace dependencies");
    let (from_version, to_version) = parse_req(req)?;
    let from_name_and_version = from_version.name + "/" + &from_version.version;
    let to_name_and_version = to_version.name + "/" + &to_version.version;

    // create read only transaction
    let ro_txn = graph.create_ro_txn()?;

    // require the start node
    let mut from_version_iter = ro_txn.vertex_index_iter_ids_from(
        "version",
        "name_and_version",
        &FieldData::String(from_name_and_version.clone()),
        &FieldData::String(from_name_and_version),
    )?;
    let from_id = from_version_iter.next().ok_or("the node start not found")?;

    // require the end node
    let mut to_version_iter = ro_txn.vertex_index_iter_ids_from(
        "version",
        "name_and_version",
        &FieldData::String(to_name_and_version.clone()),
        &FieldData::String(to_name_and_version),
    )?;
    let to_id = to_version_iter.next().ok_or("the node end not found")?;

    // BFS to search
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(from_id);

    while let Some(node_id) = queue.pop_front() {
        let mut node_cursor = ro_txn.vertex_cur()?;
        node_cursor.seek(node_id, false)?;
        assert!(node_cursor.is_valid());

        if node_id == to_id {
            return Ok("1".to_string());
        }

        let neighbors: Vec<_> = node_cursor
            .out_edge_cursor()?
            .into_edges()
            .filter(|(_, label, _)| label == "depends_on")
            .collect();

        for (id, _, _) in neighbors {
            let neighbor = id.dst;
            if visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }

    Ok("0".to_string())
}

fn parse_req(req: &str) -> Result<(Version, Version), String> {
    let v: Vec<_> = req.split(',').collect();
    if v.len() != 2 {
        return Err("parse request error".to_string());
    }
    let from: Vec<String> = v[0].split(' ').map(|x| x.to_string()).collect();
    let to: Vec<String> = v[1].split(' ').map(|x| x.to_string()).collect();

    if from.len() != 2 || to.len() != 2 {
        return Err("parse request error".to_string());
    }

    let from = Version {
        name: from[0].to_string(),
        version: from[1].to_string(),
    };
    let to = Version {
        name: to[0].to_string(),
        version: to[1].to_string(),
    };

    Ok((from, to))
}
