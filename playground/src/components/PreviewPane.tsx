// Renders the wasm HTML inside `.afm-root` (ADR-0011 brand boundary
// — `.afm-root` is the host's job, not the renderer's).
//
// `innerHTML` is safe here by construction: the source string comes
// straight from the afm pipeline this page also ships. DO NOT wrap it
// in DOMPurify — that would strip the <ruby> / <rt> tags afm emits.

import { type Accessor, type Component } from 'solid-js';

interface PreviewPaneProps {
  html: Accessor<string>;
}

const PreviewPane: Component<PreviewPaneProps> = (props) => {
  return (
    <div class="afm-pg-preview-content">
      <div class="afm-root" innerHTML={props.html()} />
    </div>
  );
};

export default PreviewPane;
