import cp from "node:child_process";

function getCommitHash(): string {
  try {
    const hash = cp
      .execSync("git rev-parse --short HEAD", { encoding: "utf-8" })
      .trim();
    return hash;
  } catch (error) {
    console.warn("Could not retrieve git commit hash:", error);
    return "unknown";
  }
}

export const commitHash = getCommitHash();
