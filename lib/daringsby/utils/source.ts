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
    chunkSize: 60,
    chunkOverlap: 0,
  });
  const jsDocs = await jsSplitter.createDocuments([JS_CODE]);
  return jsDocs;
}
