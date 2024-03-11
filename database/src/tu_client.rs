use std::error::Error;

use tugraph::{
    cursor::EdgeCursor, cursor::VertexCursor, db::Graph, field::FieldData, txn::TxnRead,
};

#[tugraph_plugin_util::tugraph_plugin]
fn movie_friend(graph: &mut Graph, req: &str) -> Result<String, Box<dyn Error>> {
    // req stores the request string from the web
    // its format is "person_name,movie_title"
    // parse from req to get person_name and movie_title
    let (person_name, movie_title) = parse_req(req)?;

    // create read only transaction
    let ro_txn = graph.create_ro_txn()?;
    let mut movie_index_iter = ro_txn.vertex_index_iter_ids_from(
        "Movie",
        "title",
        &FieldData::String(movie_title.clone()),
        &FieldData::String(movie_title),
    )?;

    let movie_id = movie_index_iter.next().ok_or("movie not found")?;

    // find the movie vertex with vid = movie_id
    let mut vertex_cur = ro_txn.vertex_cur()?;
    vertex_cur.seek(movie_id, false)?;
    // get all the watcher ids of the movie
    let watcher_ids: Vec<_> = vertex_cur.in_edge_cursor()?.into_edge_srcs().collect();

    // collect all user names through watcher ids
    let user_names = watcher_ids
        .iter()
        .map(|user_id| {
            let mut user_cur = ro_txn.vertex_cur()?;
            user_cur.seek(*user_id, false)?;
            user_cur.field("name").map(|name| match name {
                FieldData::String(name) => name,
                _ => panic!("name should be string"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // return all user names except the person_name
    Ok(user_names
        .into_iter()
        .filter(|name| name != &person_name)
        .collect::<Vec<_>>()
        .join(","))
}

fn parse_req(req: &str) -> Result<(String, String), String> {
    let v: Vec<_> = req.split(',').collect();
    if v.len() != 2 {
        return Err("parse request error, format should be `Person.name,Movie.title`".to_string());
    }
    Ok((v[0].to_string(), v[1].to_string()))
}
