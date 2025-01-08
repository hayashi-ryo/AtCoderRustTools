use reqwest::cookie::Jar;
use reqwest::Client;
use rpassword::read_password;
use scraper::{Html, Selector};
use std::{
    io::{self, Write},
    sync::Arc,
};

pub struct UserCredentials {
    pub user_id: String,
    pub password: String,
}

impl UserCredentials {
    pub fn new(user_id: String, password: String) -> Self {
        UserCredentials { user_id, password }
    }
}

pub fn get_credentials() -> Result<UserCredentials, io::Error> {
    let user_id = prompt_user("User ID: ")?;
    let password = prompt_password("Password: ")?;

    Ok(UserCredentials::new(user_id, password))
}

fn prompt_user(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_password(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let password = read_password()?;
    Ok(password)
}

pub async fn login_to_atcoder(
    credentials: &UserCredentials,
) -> Result<(), Box<dyn std::error::Error>> {
    let login_url = "https://atcoder.jp/login";

    // Cookieストアを作成
    let cookie_store = Arc::new(Jar::default());

    // Cookieストアを持つClientを作成
    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(Arc::clone(&cookie_store))
        .build()?;

    // csrf_tokenを取得
    let selector = Selector::parse("input[name=\"csrf_token\"]").unwrap();
    let body = client.get(login_url).send().await?.text().await?;
    let document = Html::parse_document(&body);
    let csrf_token = document
        .select(&selector)
        .next()
        .unwrap()
        .value()
        .attr("value")
        .unwrap()
        .to_string();

    // ログインデータの作成
    let login_form = [
        ("username", credentials.user_id.as_str()),
        ("password", credentials.password.as_str()),
        ("csrf_token", csrf_token.as_str()),
    ];

    // ログインリクエストの送信
    let login_response = client.post(login_url).form(&login_form).send().await?;
    login_response.error_for_status_ref()?;
    let response_url = login_response.url().to_string();
    if response_url == "https://atcoder.jp/home" {
        println!("ログインに成功しました！");
        Ok(())
    } else {
        println!("ログインに失敗しました:");
        Err(format!(
            "ログイン失敗: リダイレクト先が/homeではありません ({})",
            response_url
        )
        .into())
    }
}
