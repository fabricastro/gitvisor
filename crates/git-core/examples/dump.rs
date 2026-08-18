//! Prints an ASCII rendering of the computed graph. Handy for eyeballing the
//! lane algorithm against a real repository:
//!
//!     cargo run -p git-core --example dump -- /path/to/repo

use git_core::GitRepo;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let repo = GitRepo::open(&path)?;

    println!("{:#?}", repo.info()?);
    let graph = repo.graph(Some(60))?;
    println!("lanes: {}  rows: {}", graph.lane_count, graph.rows.len());

    for row in &graph.rows {
        let mut track = vec![' '; graph.lane_count as usize];
        for edge in &row.edges {
            track[edge.to_lane as usize] = '|';
        }
        track[row.lane as usize] = if row.commit.is_merge() { 'M' } else { '*' };
        let refs: Vec<_> = row.commit.refs.iter().map(|r| r.name.as_str()).collect();
        println!(
            "{} {} c{:<2} {}{}",
            track.iter().collect::<String>(),
            row.commit.short_id,
            row.color,
            row.commit.summary,
            if refs.is_empty() {
                String::new()
            } else {
                format!("  [{}]", refs.join(", "))
            }
        );
    }
    Ok(())
}
