import { Component, For, Show } from 'solid-js';
import { files } from '../../lib/store';
import { api } from '../../lib/api';
import { formatFileSize } from '../../lib/format';
import FileUpload from './FileUpload';
import styles from './FileList.module.css';

const FileList: Component = () => {
  const handleLoad = async (id: string) => {
    await api.loadFile(id);
  };

  const handleDelete = async (id: string) => {
    await api.deleteFile(id);
  };

  return (
    <div class="panel">
      <div class="panel-header">
        <span>Files</span>
      </div>
      <div class={styles.body}>
        <FileUpload />
        <Show
          when={files().length > 0}
          fallback={<div class={styles.empty}>No files uploaded</div>}
        >
          <div class={styles.list}>
            <For each={files()}>
              {(file) => (
                <div class={styles.file}>
                  <div class={styles.info}>
                    <span class={styles.name} title={file.name}>{file.name}</span>
                    <span class={styles.meta}>
                      {formatFileSize(file.size_bytes)} &middot; {file.line_count} lines
                    </span>
                  </div>
                  <div class={styles.actions}>
                    <button
                      class={styles.loadBtn}
                      onClick={() => handleLoad(file.id)}
                      title="Load file"
                    >
                      Load
                    </button>
                    <button
                      class={styles.deleteBtn}
                      onClick={() => handleDelete(file.id)}
                      title="Delete file"
                    >
                      &#x2715;
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default FileList;
