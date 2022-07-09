use actors;

const ADDR: &str = "127.0.0.1:9002";

#[actix_rt::main]
async fn main() {
    actors::run_actor(ADDR).await;
}
