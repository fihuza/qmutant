const VIEWER: &str =
    "https://cdn.jsdelivr.net/npm/mutation-testing-elements@3.9.0/dist/mutation-test-elements.js";
const VIEWER_INTEGRITY: &str =
    "sha384-/dXYuoYRvO6rJ0FNtdKR+Hmq5iGrkkSDpc/TGgbSoReog26ZqhLhpSt4l+0Wwew2";

#[must_use]
pub fn render(json: &str) -> String {
    let embedded = json.replace('<', "\\u003c");
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>qmutant report</title>
<script src="{VIEWER}" integrity="{VIEWER_INTEGRITY}" crossorigin="anonymous"></script>
</head>
<body>
<mutation-test-report-app title-postfix="qmutant"></mutation-test-report-app>
<script>
const app = document.querySelector("mutation-test-report-app");
app.report = {embedded};
const paint = () => {{ document.body.style.backgroundColor = app.themeBackgroundColor; }};
app.addEventListener("theme-changed", paint);
paint();
</script>
</body>
</html>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_inside_the_report_cannot_close_the_script() {
        let page = render(r#"{"source":"</script><b>"}"#);
        assert!(!page.contains("</script><b>"));
        assert!(page.contains(r#"app.report = {"source":"\u003c/script>\u003cb>"};"#));
    }

    #[test]
    fn the_viewer_is_pinned_and_verified() {
        let page = render("{}");
        assert!(page.contains("mutation-testing-elements@3.9.0"));
        assert!(page.contains(r#"integrity="sha384-"#));
    }
}
