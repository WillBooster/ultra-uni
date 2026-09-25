import { syntaxTree } from '../../dist/index.js';

export default {
  async fetch(request) {
    const { language, source } = await request.json();
    try {
      return Response.json({ tree: syntaxTree(language, source) });
    } catch (error) {
      return Response.json({ error: error.message }, { status: 400 });
    }
  },
};
