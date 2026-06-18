import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Markdown } from 'tiptap-markdown';

const editor = new Editor({
  extensions: [
    StarterKit,
    Link.configure({ openOnClick: false, autolink: true }),
    Placeholder.configure({ placeholder: 'test' }),
    Markdown.configure({
      html: false,
      breaks: true,
      transformPastedText: true,
      bulletListMarker: "-",
    }),
  ],
  content: "",
});

console.log('=== Initial State (empty) ===');
console.log('getMarkdown():', JSON.stringify(editor.storage.markdown.getMarkdown()));
console.log('doc.content.size:', editor.state.doc.content.size);

console.log('\n=== After setContent("", false) ===');
const updateFired = [];
const oldEmit = editor.emit;
editor.emit = function(event, ...args) {
  if (event === 'update') {
    updateFired.push(event);
    console.log('onUpdate fired!');
  }
  return oldEmit.call(this, event, ...args);
};

editor.commands.setContent("", false);
console.log('getMarkdown():', JSON.stringify(editor.storage.markdown.getMarkdown()));
console.log('Update fired during setContent("", false):', updateFired.length > 0);

editor.destroy();
