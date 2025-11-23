// Single React instance shared with react-dom; avoid bundled duplicate React.
import * as React from "https://esm.sh/react@18.3.1?dev";
import htm from "https://esm.sh/htm@3.1.1";

export { React };
export const html = htm.bind(React.createElement);
