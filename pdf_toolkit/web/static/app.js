function download(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function filenameFrom(res) {
  const disposition = res.headers.get("Content-Disposition") || "";
  const star = disposition.match(/filename\*=UTF-8''([^;]+)/i);
  if (star) {
    return decodeURIComponent(star[1]);
  }
  const plain = disposition.match(/filename="?([^";]+)"?/i);
  return plain ? plain[1] : "download.pdf";
}

function wire(formId, url) {
  const form = document.getElementById(formId);
  const error = form.querySelector(".error");

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    error.hidden = true;

    try {
      const res = await fetch(url, { method: "POST", body: new FormData(form) });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        error.textContent =
          body && body.error ? body.error : `request failed (${res.status})`;
        error.hidden = false;
        return;
      }
      const blob = await res.blob();
      download(blob, filenameFrom(res));
    } catch (err) {
      error.textContent = "request failed: " + err.message;
      error.hidden = false;
    }
  });
}

wire("compress-form", "/compress");
wire("merge-form", "/merge");
