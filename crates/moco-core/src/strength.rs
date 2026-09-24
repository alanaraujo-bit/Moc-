//! Password strength estimation (zxcvbn) tuned for Brazilian users, with Portuguese
//! feedback and crack-time phrasing.

use serde::{Deserialize, Serialize};

/// Passwords and words that top Brazilian leak lists but are absent from zxcvbn's English
/// dictionaries. Treated as user-dictionary words, so "flamengo2024" scores as weak.
const BR_COMMON: &[&str] = &[
    "senha", "senha123", "123mudar", "mudar123", "mudar", "brasil", "flamengo", "corinthians", "palmeiras",
    "saopaulo", "santos", "vasco", "gremio", "internacional", "cruzeiro", "atletico", "botafogo", "fluminense",
    "bahia", "vitoria", "sport", "fortaleza", "ceara", "amor", "amorzinho", "teamo", "jesus", "jesuscristo",
    "deus", "deusefiel", "familia", "felicidade", "saudade", "mae", "maezinha", "pai", "filho", "filha",
    "gabriel", "lucas", "mateus", "matheus", "pedro", "joao", "maria", "ana", "julia", "beatriz", "larissa",
    "fernanda", "camila", "amanda", "bruna", "leticia", "rafael", "gustavo", "felipe", "bruno", "rodrigo",
    "carlos", "eduardo", "ricardo", "daniel", "thiago", "tiago", "vinicius", "anderson", "juliana", "patricia",
    "aline", "vanessa", "renata", "cristina", "adriana", "sandra", "marcos", "paulo", "jose", "antonio",
    "francisco", "raimundo", "sebastiao", "abcd1234", "qwerty123", "asdfgh", "cachorro", "gatinho", "princesa",
    "estrela", "sol", "lua", "flor", "chocolate", "futebol", "cerveja", "caipirinha", "saudades", "obrigado",
    "bomdia", "boanoite", "novasenha", "minhasenha", "trocar", "acesso", "administrador", "admin123",
    "netflix", "spotify", "google", "gmail", "facebook", "instagram", "whatsapp", "nubank", "itau", "bradesco",
    "caixa", "santander", "ifood", "uber", "mercadolivre", "amazon", "hotmail", "outlook", "microsoft", "apple",
    "samsung", "steam", "tiktok", "twitter", "linkedin", "picpay", "shopee", "magalu",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Strength {
    /// 0 (muito fraca) … 4 (muito forte).
    pub score: u8,
    pub label: String,
    pub guesses_log10: f64,
    /// Estimated time for an offline attacker against a slow hash (10⁴ guesses/s).
    pub crack_time: String,
    pub warning: Option<String>,
    pub suggestions: Vec<String>,
}

pub fn label_for(score: u8) -> &'static str {
    match score {
        0 => "Muito fraca",
        1 => "Fraca",
        2 => "Razoável",
        3 => "Forte",
        _ => "Muito forte",
    }
}

fn translate(msg: &str) -> String {
    let m = msg.to_ascii_lowercase();
    let pairs: &[(&str, &str)] = &[
        ("straight rows of keys", "Sequências de teclas vizinhas são fáceis de adivinhar."),
        ("short keyboard patterns", "Padrões curtos do teclado são fáceis de adivinhar."),
        ("repeats like \"aaa\"", "Repetições como \"aaa\" são fáceis de adivinhar."),
        ("repeats like \"abcabcabc\"", "Repetições como \"abcabc\" quase não ajudam."),
        ("top-10", "Esta é uma das 10 senhas mais usadas do mundo."),
        ("top 10", "Esta é uma das 10 senhas mais usadas do mundo."),
        ("top-100", "Esta é uma das 100 senhas mais usadas do mundo."),
        ("top 100", "Esta é uma das 100 senhas mais usadas do mundo."),
        ("very common password", "Esta é uma senha muito comum."),
        ("common password", "Esta é uma senha comum."),
        ("similar to a commonly used", "Parece demais com uma senha muito usada."),
        ("sequences like abc", "Sequências como \"abc\" ou \"6543\" são fáceis de adivinhar."),
        ("recent years", "Anos recentes são fáceis de adivinhar."),
        ("a word by itself", "Uma palavra sozinha é fácil de adivinhar."),
        ("dates are often", "Datas costumam ser fáceis de adivinhar."),
        ("names and surnames by themselves", "Nomes e sobrenomes sozinhos são fáceis de adivinhar."),
        ("common names and surnames", "Nomes e sobrenomes comuns são fáceis de adivinhar."),
        ("use a few words", "Use algumas palavras aleatórias, sem formar uma frase conhecida."),
        ("no need for symbols", "Não precisa de símbolos, números ou maiúsculas se a senha for longa."),
        ("add another word", "Acrescente mais uma ou duas palavras incomuns."),
        ("longer keyboard pattern", "Use um padrão de teclado mais longo, com mais curvas."),
        ("avoid repeated words", "Evite palavras e caracteres repetidos."),
        ("avoid sequences", "Evite sequências."),
        ("avoid recent years", "Evite anos recentes."),
        ("avoid years that are associated", "Evite anos ligados a você."),
        ("avoid dates and years", "Evite datas e anos ligados a você."),
        ("capitalization doesn't help", "Letra maiúscula no começo quase não ajuda."),
        ("all-uppercase is almost as easy", "Tudo em maiúsculas é quase tão fácil quanto tudo em minúsculas."),
        ("reversed words aren't much harder", "Palavras de trás pra frente não são muito mais difíceis."),
        ("predictable substitutions", "Trocas previsíveis como \"@\" no lugar de \"a\" não ajudam muito."),
    ];
    for (needle, pt) in pairs {
        if m.contains(needle) {
            return (*pt).to_string();
        }
    }
    msg.to_string()
}

/// Human phrasing for a duration in seconds.
pub fn humanize_seconds(secs: f64) -> String {
    const MIN: f64 = 60.0;
    const HOUR: f64 = 3600.0;
    const DAY: f64 = 86_400.0;
    const MONTH: f64 = DAY * 30.0;
    const YEAR: f64 = DAY * 365.0;
    const CENTURY: f64 = YEAR * 100.0;
    let plural = |n: f64, one: &str, many: &str| {
        let n = n.round().max(1.0) as u64;
        if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
    };
    if secs < 1.0 {
        "instantes".into()
    } else if secs < MIN {
        plural(secs, "segundo", "segundos")
    } else if secs < HOUR {
        plural(secs / MIN, "minuto", "minutos")
    } else if secs < DAY {
        plural(secs / HOUR, "hora", "horas")
    } else if secs < MONTH {
        plural(secs / DAY, "dia", "dias")
    } else if secs < YEAR {
        plural(secs / MONTH, "mês", "meses")
    } else if secs < CENTURY {
        plural(secs / YEAR, "ano", "anos")
    } else {
        "séculos".into()
    }
}

pub fn estimate(password: &str, user_inputs: &[&str]) -> Strength {
    if password.is_empty() {
        return Strength {
            score: 0,
            label: label_for(0).into(),
            guesses_log10: 0.0,
            crack_time: "instantes".into(),
            warning: None,
            suggestions: vec![],
        };
    }
    // zxcvbn is quadratic-ish in length; very long passphrases are strong regardless.
    let sample: String = password.chars().take(100).collect();
    let mut inputs: Vec<&str> = BR_COMMON.to_vec();
    inputs.extend_from_slice(user_inputs);
    let e = zxcvbn::zxcvbn(&sample, &inputs);
    let score = u8::from(e.score());
    let guesses_log10 = e.guesses_log10();
    let secs = 10f64.powf(guesses_log10) / 1e4;
    let (warning, suggestions) = match e.feedback() {
        Some(fb) => (
            fb.warning().map(|w| translate(&w.to_string())),
            fb.suggestions().iter().map(|s| translate(&s.to_string())).collect(),
        ),
        None => (None, vec![]),
    };
    Strength { score, label: label_for(score).into(), guesses_log10, crack_time: humanize_seconds(secs), warning, suggestions }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brazilian_classics_are_weak() {
        for p in ["senha123", "flamengo", "123mudar", "Corinthians2024"] {
            assert!(estimate(p, &[]).score <= 1, "{p} scored too high");
        }
    }

    #[test]
    fn strong_passwords_score_high() {
        let s = estimate("Tatu-Caju-Varanda7-Moqueca", &[]);
        assert!(s.score >= 3, "{s:?}");
        assert_eq!(estimate("q8#Lm2!vZr9@Tx4^", &[]).score, 4);
    }

    #[test]
    fn humanized_times() {
        assert_eq!(humanize_seconds(0.2), "instantes");
        assert_eq!(humanize_seconds(90.0), "2 minutos");
        assert_eq!(humanize_seconds(3600.0 * 5.0), "5 horas");
        assert_eq!(humanize_seconds(1e12), "séculos");
    }
}
