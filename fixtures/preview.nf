// Previews a web page: its title, read out of the page with string methods, and its
// type, from the response's headers.

state address = ""
state status = ""
state page = ""
state code = 0
state headers: [string] = []
state title = ""
state kind = ""

// The text between `open` and `close` in `text`, or "" when either is missing.
fn between(text: string, open: string, close: string) -> string {
  let start = text.find(open)
  if start < 0 {
    return ""
  }
  let rest = text.slice(start + open.len(), text.len())
  let end = rest.find(close)
  if end < 0 {
    return ""
  }
  return rest.slice(0, end).trim()
}

// The value of the response header `name`, written in lower case.
fn header(name: string) -> string {
  let prefix = "{name}: "
  for line in headers {
    if line.starts_with(prefix) {
      return line.slice(prefix.len(), line.len())
    }
  }
  return ""
}

fn preview() {
  title = ""
  kind = ""
  fetch(address, status: $status, text: $page, code: $code, response_headers: $headers) {
    title = between(page, "<title>", "</title>")
    kind = header("content-type")
  }
}

window("Preview", width: 480, height: 240) {
  column(padding: 20, spacing: 12) {
    row(spacing: 8, align: .center) {
      column(grow: true) {
        text_field($address, placeholder: "https://example.com") { preview() }
      }
      button("Preview", style: .primary) { preview() }
    }
    label(status, style: .caption)
    if title != "" {
      label(title, style: .title)
    }
    if kind != "" {
      label("{kind} · {page.len()} characters · HTTP {code}", style: .caption)
    }
  }
}
