// VITE_API_URL is substituted at build time, so it has to be set wherever `vite build` runs.
const api_url: string = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

const out = document.getElementById("out")!;
document.getElementById("hit")!.addEventListener("click", async () => {
  try {
    const res = await fetch(`${api_url}/hit`);
    out.textContent = JSON.stringify(await res.json(), null, 2);
  } catch (e) {
    out.textContent = `failed to reach ${api_url}: ${e}`;
  }
});
