import * as rest from "https://cdn.skypack.dev/@octokit/rest";

// Initialize Octokit instance
const octokit = new rest.Octokit();

// Define a simple API for the agent to interact with the GitHub repository
export const GitInspector = {
    // Function to list all files in the repository
    async listFiles(repoOwner: string, repoName: string): Promise<string[]> {
        const response = await octokit.request(
            "GET /repos/{owner}/{repo}/contents/{path}",
            {
                owner: repoOwner,
                repo: repoName,
                path: "",
            },
        );
        const files = Array.isArray(response.data) ? response.data : [];
        return files.filter((file: { type: string }) => file.type === "file")
            .map((
                file: { path: string },
            ) => file.path);
    },

    // Function to fetch the content of a specific file
    async fetchFileContent(
        repoOwner: string,
        repoName: string,
        filePath: string,
    ): Promise<string> {
        const response = await octokit.request(
            "GET /repos/{owner}/{repo}/contents/{path}",
            {
                owner: repoOwner,
                repo: repoName,
                path: filePath,
            },
        );
        if (response.data.type !== "file" || !("content" in response.data)) {
            throw new Error(`Failed to fetch file content`);
        }
        return atob(response.data.content);
    },

    // Function to split content into smaller chunks
    splitIntoChunks(sourceCode: string, chunkSize: number = 5): string[] {
        const lines = sourceCode.split("\n");
        const chunks = [];
        for (let i = 0; i < lines.length; i += chunkSize) {
            chunks.push(lines.slice(i, i + chunkSize).join("\n"));
        }
        return chunks;
    },
};

// This API is simple and intuitive:
// - GitInspector.listFiles(repoOwner, repoName): List all files in the repository.
// - GitInspector.fetchFileContent(repoOwner, repoName, filePath): Fetch the content of a specific file.
// - GitInspector.splitIntoChunks(sourceCode, chunkSize): Split content into manageable chunks.
// The agent can call these functions to traverse and process the repository in small parts.
