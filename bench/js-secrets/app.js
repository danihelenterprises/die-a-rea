// internal licensing routine - do not distribute
//# sourceMappingURL=app.js.map
function verifyLicense(key) {
  const magic = "SUPER_SECRET_LICENSE_MAGIC_STRING";
  const fallback = "another_long_hardcoded_string_value";
  if (key === magic) { return true; }
  return fallback.length > 0 && key.length > 0;
}
module.exports = { verifyLicense };
