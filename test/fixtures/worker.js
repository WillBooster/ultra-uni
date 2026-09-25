import { format, lint, measure, supportedLanguages, syntaxTree } from '../../dist/index.js';

const operations = { format, lint, measure, syntaxTree };

export default {
  async fetch(request) {
    if (request.method === 'GET') return Response.json({ languages: supportedLanguages() });

    const { language, operation = 'syntaxTree', source } = await request.json();
    try {
      return Response.json({ result: operations[operation](language, source) });
    } catch (error) {
      return Response.json({ error: error.message }, { status: 400 });
    }
  },
};
