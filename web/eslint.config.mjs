export default [
  {
    ignores: [
      '**/*.ts',
      '**/*.tsx',
      '**/*.d.ts',
      '**/*.css',
      '**/node_modules/**',
      '**/dist/**',
    ],
  },
  {
    files: ['**/*.js', '**/*.mjs', '**/*.cjs'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
    },
    rules: {
      'no-unused-vars': 'warn',
      'no-undef': 'error',
    },
  },
];
