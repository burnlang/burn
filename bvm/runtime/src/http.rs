use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

struct Url {
    https: bool,
    host: String,
    port: u16,
    path: String,
}

fn parse_url(url: &str) -> Result<Url, String> {
    let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("unsupported URL: {}", url));
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => match rest.find('?') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        },
    };
    let path = if path.starts_with('?') { format!("/{}", path) } else { path.to_string() };
    let (host, port) = match hostport.rfind(':') {
        Some(i) if !hostport[i + 1..].is_empty() && hostport[i + 1..].chars().all(|c| c.is_ascii_digit()) => {
            (hostport[..i].to_string(), hostport[i + 1..].parse().unwrap_or(80))
        }
        _ => (hostport.to_string(), if https { 443 } else { 80 }),
    };
    if host.is_empty() {
        return Err(format!("invalid URL: {}", url));
    }
    Ok(Url { https, host, port, path })
}

fn split_response(raw: &[u8]) -> (i64, String, Vec<u8>) {
    let mut rest = raw;
    loop {
        let pos = rest.windows(4).position(|w| w == b"\r\n\r\n");
        let (head, body) = match pos {
            Some(p) => (&rest[..p], &rest[p + 4..]),
            None => (rest, &rest[rest.len()..]),
        };
        let head_s = String::from_utf8_lossy(head).into_owned();
        let status = head_s
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let first = head_s.lines().next().unwrap_or("").to_ascii_lowercase();
        let tunnel = first.contains("connection established");
        if (status / 100 == 1 || tunnel) && body.starts_with(b"HTTP/") {
            rest = body;
            continue;
        }
        let headers: Vec<&str> = head_s.lines().skip(1).collect();
        return (status, headers.join("\n"), body.to_vec());
    }
}

fn dechunk(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let line_end = match body[i..].windows(2).position(|w| w == b"\r\n") {
            Some(p) => i + p,
            None => break,
        };
        let size_s = String::from_utf8_lossy(&body[i..line_end]);
        let size = usize::from_str_radix(size_s.split(';').next().unwrap_or("0").trim(), 16).unwrap_or(0);
        i = line_end + 2;
        if size == 0 {
            break;
        }
        let end = (i + size).min(body.len());
        out.extend_from_slice(&body[i..end]);
        i = end + 2;
    }
    out
}

fn plain(method: &str, u: &Url, body: &[u8], headers: &[String]) -> Result<(i64, String, String), String> {
    let mut s = TcpStream::connect((u.host.as_str(), u.port)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
    let mut req = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", method, u.path, u.host);
    let has = |name: &str| headers.iter().any(|h| h.to_ascii_lowercase().starts_with(&format!("{}:", name)));
    if !has("user-agent") {
        req.push_str("User-Agent: BurnLang/26.1.0-experimental-3\r\n");
    }
    for h in headers {
        req.push_str(h);
        req.push_str("\r\n");
    }
    if !body.is_empty() || method != "GET" {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(body).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let (status, hdrs, mut b) = split_response(&raw);
    if hdrs.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        b = dechunk(&b);
    }
    Ok((status, String::from_utf8_lossy(&b).into_owned(), hdrs))
}

fn curl(method: &str, url: &str, body: &[u8], headers: &[String]) -> Result<(i64, String, String), String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("curl");
    cmd.arg("-sS").arg("-i").arg("-X").arg(method);
    let has_ua = headers.iter().any(|h| h.to_ascii_lowercase().starts_with("user-agent:"));
    if !has_ua {
        cmd.arg("-H").arg("User-Agent: BurnLang/26.1.0-experimental-3");
    }
    for h in headers {
        cmd.arg("-H").arg(h);
    }
    if !body.is_empty() {
        cmd.arg("--data-binary").arg("@-");
    }
    cmd.arg(url);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("https requires curl: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(body);
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() && out.stdout.is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let (status, hdrs, b) = split_response(&out.stdout);
    Ok((status, String::from_utf8_lossy(&b).into_owned(), hdrs))
}

pub fn request(method: &str, url: &str, body: &[u8], headers: &[String]) -> (i64, String, String) {
    let method = method.to_ascii_uppercase();
    let r = match parse_url(url) {
        Ok(u) => {
            if u.https {
                curl(&method, url, body, headers)
            } else {
                plain(&method, &u, body, headers)
            }
        }
        Err(e) => Err(e),
    };
    match r {
        Ok(v) => v,
        Err(e) => (0, e, String::new()),
    }
}
