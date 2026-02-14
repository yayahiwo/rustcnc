import { Component, createSignal } from 'solid-js';
import { api } from '../../lib/api';
import styles from './FileUpload.module.css';

const FileUpload: Component = () => {
  const [uploading, setUploading] = createSignal(false);
  let inputRef: HTMLInputElement | undefined;

  const handleClick = () => {
    inputRef?.click();
  };

  const handleFile = async (e: Event) => {
    const target = e.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    setUploading(true);
    try {
      await api.uploadFile(file);
    } catch (err) {
      console.error('Upload failed:', err);
    } finally {
      setUploading(false);
      if (inputRef) inputRef.value = '';
    }
  };

  const handleDrop = async (e: DragEvent) => {
    e.preventDefault();
    const file = e.dataTransfer?.files?.[0];
    if (!file) return;

    setUploading(true);
    try {
      await api.uploadFile(file);
    } catch (err) {
      console.error('Upload failed:', err);
    } finally {
      setUploading(false);
    }
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
  };

  return (
    <div
      class={styles.dropzone}
      classList={{ [styles.active]: uploading() }}
      onClick={handleClick}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
    >
      <input
        ref={inputRef}
        type="file"
        accept=".gcode,.nc,.ngc,.tap,.txt"
        class={styles.hidden}
        onChange={handleFile}
      />
      <span class={styles.label}>
        {uploading() ? 'Uploading...' : 'Drop G-code file or click to upload'}
      </span>
    </div>
  );
};

export default FileUpload;
