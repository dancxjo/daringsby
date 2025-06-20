import { RecursiveCharacterTextSplitter } from "npm:@langchain/textsplitters";
import { DirectoryLoader } from "npm:langchain/document_loaders/fs/directory";
import {
  JSONLinesLoader,
  JSONLoader,
} from "npm:langchain/document_loaders/fs/json";
import { TextLoader } from "npm:langchain/document_loaders/fs/text";

export async function loadDocuments() {
  const loader = new DirectoryLoader(
    import.meta.dirname ?? process.cwd(),
    {
      ".ts": (path) => new TextLoader(path),
      ".md": (path) => new TextLoader(path),
      ".tsx": (path) => new TextLoader(path),
      ".css": (path) => new TextLoader(path),
      ".json": (path) => new JSONLoader(path),
      ".yaml": (path) => new JSONLoader(path),
      ".yml": (path) => new JSONLoader(path),
      ".": (path) => new TextLoader(path),
      ".sh": (path) => new TextLoader(path),
      ".conf": (path) => new TextLoader(path),
    },
  );
  const docs = await loader.load();
  return (await Promise.all(
    docs.map((doc) => createSourceCodeDocument(doc.pageContent)),
  ))
    .flat();
}

export async function createSourceCodeDocument(JS_CODE: string) {
  const jsSplitter = RecursiveCharacterTextSplitter.fromLanguage("js", {
    chunkSize: 512,
    chunkOverlap: 4,
  });
  const jsDocs = await jsSplitter.createDocuments([JS_CODE]);
  jsDocs.sort((a, b) => Math.random() - 0.5);
  return jsDocs;
}
