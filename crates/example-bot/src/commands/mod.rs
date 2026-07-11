mod echo;
mod ping;

pub fn reply(command: &str, args: &[String]) -> Option<String> {
    match command {
        "ping" => Some(ping::reply()),
        "echo" => Some(echo::reply(args)),
        _ => None,
    }
}
