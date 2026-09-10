use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

const COURSES_URL: &str = "https://querycourse.ntust.edu.tw/querycourse/api/courses";
const SEMESTERS_URL: &str = "https://querycourse.ntust.edu.tw/querycourse/api/semestersinfo";

#[derive(Serialize)]
struct CourseQuery {
    #[serde(rename = "CourseNo")]
    course_no: String,
    #[serde(rename = "Language")]
    language: String,
    #[serde(rename = "Semester")]
    semester: String,
}

#[derive(Debug, Deserialize)]
pub struct Course {
    #[serde(rename = "CourseName", default)]
    pub course_name: String,
    #[serde(rename = "CourseNo", default)]
    pub course_no: String,
    #[serde(
        rename = "Restrict2",
        default,
        deserialize_with = "de_i32_from_string_or_int"
    )]
    pub restrict2: i32,
    #[serde(
        rename = "ChooseStudent",
        default,
        deserialize_with = "de_i32_from_string_or_int"
    )]
    pub choose_student: i32,
}

impl Course {
    pub fn has_seat(&self) -> bool {
        self.restrict2 - self.choose_student > 0
    }

    pub fn message(&self) -> String {
        format!(
            "{} {} ({}/{})",
            self.course_name, self.course_no, self.choose_student, self.restrict2
        )
    }
}

fn de_i32_from_string_or_int<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::Number(n) => n
            .as_i64()
            .and_then(|x| i32::try_from(x).ok())
            .ok_or_else(|| serde::de::Error::custom(format!("invalid number: {n}"))),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(0);
            }
            s.parse::<i32>()
                .map_err(|_| serde::de::Error::custom(format!("invalid number string: {s}")))
        }
        Value::Null => Ok(0),
        other => Err(serde::de::Error::custom(format!(
            "expected number or string, got: {other}"
        ))),
    }
}

pub async fn get_semester(
    client: &Client,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let data = client
        .get(SEMESTERS_URL)
        .send()
        .await?
        .json::<Value>()
        .await?;
    let body = data
        .get(0)
        .and_then(|row| row.get("Semester"))
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .unwrap_or_default();
    if body.is_empty() {
        return Err("empty Semester from semestersinfo".into());
    }
    Ok(body)
}

pub async fn fetch_course(
    client: &Client,
    semester: &str,
    course_no: &str,
) -> Result<Course, Box<dyn std::error::Error + Send + Sync>> {
    let payload = CourseQuery {
        course_no: course_no.to_string(),
        language: "zh".to_string(),
        semester: semester.to_string(),
    };
    let courses = client
        .post(COURSES_URL)
        .json(&payload)
        .send()
        .await?
        .json::<Vec<Course>>()
        .await?;
    courses
        .into_iter()
        .next()
        .ok_or_else(|| format!("查無課程: {course_no}").into())
}
