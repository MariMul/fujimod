import ts from 'typescript-api';

/** Camera methods declared by the generated bindings or defined by their loader. */
export function cameraApi(source: string): Set<string> {
  const methods = new Set<string>();
  const file = ts.createSourceFile('camera.ts', source, ts.ScriptTarget.Latest, true);
  function visit(node: ts.Node) {
    if (
      (ts.isClassDeclaration(node) || ts.isClassExpression(node)) &&
      node.name?.text === 'Camera'
    ) {
      for (const member of node.members) {
        if (!ts.isMethodDeclaration(member) || !ts.isIdentifier(member.name)) continue;
        const isStatic = member.modifiers?.some(
          (modifier) => modifier.kind === ts.SyntaxKind.StaticKeyword,
        );
        methods.add(`${isStatic ? 'static ' : ''}${member.name.text}`);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(file);
  return methods;
}

/** Refuse a runtime that lacks methods used by the website's generated API. */
export function validateRuntimeApi(loader: string, declarations: string) {
  const expected = cameraApi(declarations);
  if (!expected.size) throw Error('Generated Camera API has no methods.');
  const actual = cameraApi(loader);
  const missing = [...expected].filter((method) => !actual.has(method));
  if (missing.length) throw Error(`Runtime Camera API is missing: ${missing.join(', ')}`);
}
