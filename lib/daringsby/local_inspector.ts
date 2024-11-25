// Define a simple API for the agent to interact with the local file system
export const LocalFolderInspector = {
  // Function to list all files in the directory
  async listFiles(directoryPath: string): Promise<string[]> {
    // Normalize paths to ensure consistency
    // directoryPath = directoryPath.replace(/^\/|^\.\/|^$/, "");
    try {
      const entries = [];
      for await (const entry of Deno.readDir(directoryPath)) {
        // if (entry.isFile) {
        entries.push(entry.name);
        // }
      }
      return entries;
    } catch (error) {
      throw new Error(`Failed to list files in directory: ${error.message}`);
    }
  },

  // Function to fetch the content of a specific file
  async fetchFileContent(filePath: string): Promise<string> {
    // Normalize paths to ensure consistency
    // filePath = filePath.replace(/^\/|^\.\/|^$/, "");
    try {
      const content = await Deno.readTextFile(filePath);
      return content;
    } catch (error) {
      throw new Error(`Failed to fetch file content: ${error.message}`);
    }
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

// The agent can use the following functions to interact with the folder containing this file:
// - LocalFolderInspector.listFiles(currentDirectory): List all files in the current directory.
// - LocalFolderInspector.fetchFileContent(join(currentDirectory, "exampleFile.ts")): Fetch the content of a specific file.
// - LocalFolderInspector.splitIntoChunks(fileContent, chunkSize): Split content into manageable chunks.
// This allows the agent to traverse and process files in the local folder in small parts.
