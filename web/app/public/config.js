// Where the site finds its two backends. The hosted site's API sits behind
// the same CloudFront address under /api; a local Satchel listens on 47800.
window.SATCHEL_CONFIG = {
  awsApi: "/api",
  localApi: "http://127.0.0.1:47800",
  installUrl: "https://raw.githubusercontent.com/frectrium/Satchel/main/scripts/install.sh",
};
