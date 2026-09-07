use dns_lookup::{lookup_addr, lookup_host};
use hbb_common::{bail, ResultType};
use sodiumoxide::crypto::sign;
use std::{
    env,
    net::{IpAddr, TcpStream},
    process, str,
};

fn print_help() {
    println!(
        "Usage:
    rustdesk-utils [command]\n
Available Commands:
    genkeypair                                   Generate a new keypair
    validatekeypair [public key] [secret key]    Validate an existing keypair
    doctor [rustdesk-server]                     Check for server connection problems
    hashtoken [token]                            Hash an admin API token for ADMIN_API_TOKEN_HASH
    initadmin [env-file] [--force]                Generate a new admin token + JWT secret and write
                                                  ADMIN_API_TOKEN_HASH/ADMIN_API_JWT_SECRET into
                                                  [env-file] (default: .env in the current directory,
                                                  which is what hbbs/hbbr load on startup). Prints the
                                                  plaintext admin token once. Refuses to overwrite an
                                                  already-configured token unless --force is given."
    );
    process::exit(0x0001);
}

fn error_then_help(msg: &str) {
    println!("ERROR: {msg}\n");
    print_help();
}

fn gen_keypair() {
    let (pk, sk) = sign::gen_keypair();
    let public_key = base64::encode(pk);
    let secret_key = base64::encode(sk);
    println!("Public Key:  {public_key}");
    println!("Secret Key:  {secret_key}");
}

fn hash_token(token: &str) {
    // Admin-presence customization for this Windows client version: generate
    // the bcrypt hash stored in ADMIN_API_TOKEN_HASH on the server.
    match bcrypt::hash(token, bcrypt::DEFAULT_COST) {
        Ok(hash) => {
            println!("ADMIN_API_TOKEN_HASH={hash}");
        }
        Err(e) => {
            println!("ERROR: failed to hash token: {e}");
            process::exit(0x0001);
        }
    }
}

// Admin-presence customization: hex-encode raw random bytes. Hex (rather than
// base64) avoids any escaping concerns wherever the value ends up (shell,
// .env/ini parsing, JSON request bodies, Docker Compose `$`-interpolation).
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// Admin-presence customization: merge a single KEY=VALUE pair into a `.env`
// (INI-style) file, preserving every other existing line untouched. Used
// instead of `rust-ini`'s writer so that comments and unrelated keys already
// in the file (e.g. left by an admin, or other keys hbbs/hbbr read from
// `.env`) are never reformatted or dropped.
fn set_env_var(path: &str, key: &str, value: &str) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut found = false;
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if !found && !trimmed.starts_with('#') && !trimmed.starts_with(';') {
                if let Some(eq) = line.find('=') {
                    if line[..eq].trim().eq_ignore_ascii_case(key) {
                        found = true;
                        return format!("{key}={value}");
                    }
                }
            }
            line.to_string()
        })
        .collect();
    if !found {
        lines.push(format!("{key}={value}"));
    }
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    std::fs::write(path, content)
}

// Admin-presence customization: true if `path` already has a non-empty
// ADMIN_API_TOKEN_HASH line, used to avoid silently invalidating an
// already-distributed admin token on a re-run.
fn env_has_admin_token_hash(path: &str) -> bool {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .any(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                return false;
            }
            match line.split_once('=') {
                Some((k, v)) => {
                    k.trim().eq_ignore_ascii_case("ADMIN_API_TOKEN_HASH") && !v.trim().is_empty()
                }
                None => false,
            }
        })
}

// Admin-presence customization: generate a fresh admin token + JWT secret and
// write them (as `ADMIN_API_TOKEN_HASH`/`ADMIN_API_JWT_SECRET`) into an `.env`
// file in the format hbbs/hbbr already load from their working directory on
// startup (see `common::init_args`), so no manual bcrypt-hashing or hand
// edited Compose/systemd config is required to get the admin API running.
fn init_admin(env_path: &str, force: bool) {
    if !force && env_has_admin_token_hash(env_path) {
        println!("ERROR: '{env_path}' already has an ADMIN_API_TOKEN_HASH configured.");
        println!(
            "Re-running this would invalidate the current admin token for every already-configured client."
        );
        println!(
            "If you really want to replace it, re-run with: rustdesk-utils initadmin {env_path} --force"
        );
        process::exit(0x0001);
    }

    let token = to_hex(&sodiumoxide::randombytes::randombytes(32));
    let jwt_secret = to_hex(&sodiumoxide::randombytes::randombytes(32));

    let hash = match bcrypt::hash(&token, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            println!("ERROR: failed to hash generated token: {e}");
            process::exit(0x0001);
        }
    };

    if let Err(e) = set_env_var(env_path, "ADMIN_API_TOKEN_HASH", &hash) {
        println!("ERROR: failed to write '{env_path}': {e}");
        process::exit(0x0001);
    }
    if let Err(e) = set_env_var(env_path, "ADMIN_API_JWT_SECRET", &jwt_secret) {
        println!("ERROR: failed to write '{env_path}': {e}");
        process::exit(0x0001);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(env_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(env_path, perms);
        }
    }

    println!("Admin presence configured in '{env_path}'.\n");
    println!("=======================================================================");
    println!("  ADMIN TOKEN — give this to admins to log in. SAVE IT NOW: it cannot");
    println!("  be recovered later, only its bcrypt hash is stored.\n");
    println!("    {token}\n");
    println!("=======================================================================\n");
    println!("Written to '{env_path}':");
    println!("  ADMIN_API_TOKEN_HASH={hash}");
    println!("  ADMIN_API_JWT_SECRET={jwt_secret}\n");
    println!("Restart hbbs/hbbr (or the container/service) to load this config, e.g.:");
    println!("  docker compose restart hbbs hbbr                     # Docker Compose");
    println!("  sudo systemctl restart rustdesk-hbbs rustdesk-hbbr   # systemd/.deb install");
}

fn validate_keypair(pk: &str, sk: &str) -> ResultType<()> {
    let sk1 = base64::decode(sk);
    if sk1.is_err() {
        bail!("Invalid secret key");
    }
    let sk1 = sk1.unwrap();

    let secret_key = sign::SecretKey::from_slice(sk1.as_slice());
    if secret_key.is_none() {
        bail!("Invalid Secret key");
    }
    let secret_key = secret_key.unwrap();

    let pk1 = base64::decode(pk);
    if pk1.is_err() {
        bail!("Invalid public key");
    }
    let pk1 = pk1.unwrap();

    let public_key = sign::PublicKey::from_slice(pk1.as_slice());
    if public_key.is_none() {
        bail!("Invalid Public key");
    }
    let public_key = public_key.unwrap();

    let random_data_to_test = b"This is meh.";
    let signed_data = sign::sign(random_data_to_test, &secret_key);
    let verified_data = sign::verify(&signed_data, &public_key);
    if verified_data.is_err() {
        bail!("Key pair is INVALID");
    }
    let verified_data = verified_data.unwrap();

    if random_data_to_test != &verified_data[..] {
        bail!("Key pair is INVALID");
    }

    Ok(())
}

fn doctor_tcp(address: std::net::IpAddr, port: &str, desc: &str) {
    let start = std::time::Instant::now();
    let conn = format!("{address}:{port}");
    if let Ok(_stream) = TcpStream::connect(conn.as_str()) {
        let elapsed = std::time::Instant::now().duration_since(start);
        println!(
            "TCP Port {} ({}): OK in {} ms",
            port,
            desc,
            elapsed.as_millis()
        );
    } else {
        println!("TCP Port {port} ({desc}): ERROR");
    }
}

fn doctor_ip(server_ip_address: std::net::IpAddr, server_address: Option<&str>) {
    println!("\nChecking IP address: {server_ip_address}");
    println!("Is IPV4: {}", server_ip_address.is_ipv4());
    println!("Is IPV6: {}", server_ip_address.is_ipv6());

    // reverse dns lookup
    // TODO: (check) doesn't seem to do reverse lookup on OSX...
    let reverse = lookup_addr(&server_ip_address).unwrap();
    if let Some(server_address) = server_address {
        if reverse == server_address {
            println!("Reverse DNS lookup: '{reverse}' MATCHES server address");
        } else {
            println!(
                "Reverse DNS lookup: '{reverse}' DOESN'T MATCH server address '{server_address}'"
            );
        }
    }

    // TODO: ICMP ping?

    // port check TCP (UDP is hard to check)
    doctor_tcp(server_ip_address, "21114", "API");
    doctor_tcp(server_ip_address, "21115", "hbbs extra port for nat test");
    doctor_tcp(server_ip_address, "21116", "hbbs");
    doctor_tcp(server_ip_address, "21117", "hbbr tcp");
    doctor_tcp(server_ip_address, "21118", "hbbs websocket");
    doctor_tcp(server_ip_address, "21119", "hbbr websocket");

    // TODO: key check
}

fn doctor(server_address_unclean: &str) {
    let server_address3 = server_address_unclean.trim();
    let server_address2 = server_address3.to_lowercase();
    let server_address = server_address2.as_str();
    println!("Checking server:  {server_address}\n");
    if let Ok(server_ipaddr) = server_address.parse::<IpAddr>() {
        // user requested an ip address
        doctor_ip(server_ipaddr, None);
    } else {
        // the passed string is not an ip address
        let ips: Vec<std::net::IpAddr> = lookup_host(server_address).unwrap();
        println!("Found {} IP addresses: ", ips.len());

        ips.iter().for_each(|ip| println!(" - {ip}"));

        ips.iter()
            .for_each(|ip| doctor_ip(*ip, Some(server_address)));
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() <= 1 {
        print_help();
    }

    let command = args[1].to_lowercase();
    match command.as_str() {
        "genkeypair" => gen_keypair(),
        "validatekeypair" => {
            if args.len() <= 3 {
                error_then_help("You must supply both the public and the secret key");
            }
            let res = validate_keypair(args[2].as_str(), args[3].as_str());
            if let Err(e) = res {
                println!("{e}");
                process::exit(0x0001);
            }
            println!("Key pair is VALID");
        }
        "doctor" => {
            if args.len() <= 2 {
                error_then_help("You must supply the rustdesk-server address");
            }
            doctor(args[2].as_str());
        }
        "hashtoken" => {
            if args.len() <= 2 {
                error_then_help("You must supply the token to hash");
            }
            hash_token(args[2].as_str());
        }
        "initadmin" => {
            let mut env_path = ".env".to_string();
            let mut force = false;
            for a in &args[2..] {
                if a == "--force" || a == "-f" {
                    force = true;
                } else {
                    env_path = a.clone();
                }
            }
            init_admin(&env_path, force);
        }
        _ => print_help(),
    }
}
