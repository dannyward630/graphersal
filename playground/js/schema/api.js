// The schema editor's ONLY way to the engine: the seven schema methods, nothing else. In 0.1.0
// they go to the WebAssembly engine (engine.js `getSchema`, ...); in 0.2.0 the same editor can run
// against the server API by passing another object with these seven methods.
//
// Schemas and patches are plain objects in the schema format (book: "Schemas"). Every method
// returns a Promise; a rejected one carries the engine's located diagnostic as its message
// (`Invalid schema at /vertices/person/...`, the aggregated violation report, ...).

/**
 * @typedef {Object} SchemaApi
 * @property {() => Promise<Object|null>} get                 `get_schema`: the stored schema, or null.
 * @property {(schema: Object, mode?: string) => Promise<*>} set  `set_schema`: replaces the stored
 *   schema; `mode` ('none' | 'open' | 'closed') replaces the schema's own mode.
 * @property {(patch: Object) => Promise<Object>} patch        `patch_schema`: JSON Merge Patch; the new schema.
 * @property {(schema: Object) => Promise<Object>} validate    `validate_schema`: `{violations, diff, compatible}`.
 * @property {(patch: Object) => Promise<Object>} validatePatch `validate_schema_patch`: the same for a patch.
 * @property {() => Promise<Object>} infer                     `infer_schema`: what the data satisfies.
 * @property {(schema: Object) => Promise<{changes: Object[], compatible: boolean}>} diff  `diff_schema`.
 */

/**
 * The adapter over a playground engine (engine.js).
 * @returns {SchemaApi}
 */
export function engineSchemaApi(engine) {
  return {
    get: () => engine.getSchema(),
    set: (schema, mode) => engine.setSchema(schema, mode),
    patch: (patch) => engine.patchSchema(patch),
    validate: (schema) => engine.validateSchema(schema),
    validatePatch: (patch) => engine.validateSchemaPatch(patch),
    infer: () => engine.inferSchema(),
    diff: (schema) => engine.diffSchema(schema),
  };
}
