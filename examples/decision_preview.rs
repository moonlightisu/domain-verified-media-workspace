use domain_verified_media_workspace::workspace_join::email_belongs_to_domain;

fn main() {
    let email = "editor@stream.example";
    let domain = "stream.example";
    println!("email={email} domain={domain} eligible={}", email_belongs_to_domain(email, domain));
}

