// Turns a drop or a picker selection into files with their paths, walking
// into folders. A dropped folder arrives as an unreadable "file", so its
// entries have to be read through the FileSystem entry API instead.

export type Picked = { file: File; path: string };

/** Files the OS adds to folders, which nobody means to upload. */
const JUNK = new Set([".DS_Store", "Thumbs.db", "desktop.ini"]);

function readFile(entry: FileSystemFileEntry): Promise<File> {
  return new Promise((resolve, reject) => entry.file(resolve, reject));
}

function readBatch(reader: FileSystemDirectoryReader): Promise<FileSystemEntry[]> {
  return new Promise((resolve, reject) => reader.readEntries(resolve, reject));
}

async function walk(entry: FileSystemEntry, prefix: string, out: Picked[]) {
  if (entry.isFile) {
    if (JUNK.has(entry.name)) return;
    const file = await readFile(entry as FileSystemFileEntry);
    out.push({ file, path: prefix + file.name });
  } else if (entry.isDirectory) {
    const reader = (entry as FileSystemDirectoryEntry).createReader();
    // readEntries returns at most ~100 entries per call, so read until empty.
    for (;;) {
      const batch = await readBatch(reader);
      if (batch.length === 0) break;
      for (const child of batch) await walk(child, `${prefix}${entry.name}/`, out);
    }
  }
}

/**
 * Everything in a drop, folders included. The entries must be taken while
 * the drop event is still running, so call this synchronously in the handler.
 */
export function fromDrop(dt: DataTransfer): Promise<Picked[]> {
  const entries = [...dt.items]
    .filter((item) => item.kind === "file")
    .map((item) => item.webkitGetAsEntry?.())
    .filter((e): e is FileSystemEntry => !!e);
  const plain = [...dt.files];
  return (async () => {
    if (entries.length === 0) return plain.map((file) => ({ file, path: file.name }));
    const out: Picked[] = [];
    for (const entry of entries) await walk(entry, "", out);
    return out;
  })();
}

/** Files from an <input type="file">, with folder paths if a folder was chosen. */
export function fromInput(files: FileList | null): Picked[] {
  return [...(files ?? [])]
    .filter((file) => !JUNK.has(file.name))
    .map((file) => ({ file, path: file.webkitRelativePath || file.name }));
}
