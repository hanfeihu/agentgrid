#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
function listJsonFiles(dir) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const absolutePath = path.join(dir, entry.name);
      if (entry.isDirectory()) return listJsonFiles(absolutePath);
      if (!entry.isFile() || !entry.name.endsWith(".json")) return [];
      return [path.relative(repoRoot, absolutePath)];
    })
    .sort();
}

const schemaFiles = listJsonFiles(path.join(repoRoot, "schemas"));

let failed = false;

for (const relativePath of schemaFiles) {
  const absolutePath = path.join(repoRoot, relativePath);

  try {
    const source = fs.readFileSync(absolutePath, "utf8");
    JSON.parse(source);
    console.log(`ok ${relativePath}`);
  } catch (error) {
    failed = true;
    console.error(`fail ${relativePath}: ${error.message}`);
  }
}

if (failed) {
  process.exit(1);
}

console.log(`validated ${schemaFiles.length} schema files`);
