// Compact test fixtures use (operation, problem) notation. Encode the shared
// snapshot envelope and inspect its response independently of cache updates.
export function encode(value) {
  if (value?.type && value.data?.problem) {
    const { problem, maximum, ...data } = value.data
    const unit = ['maximize', 'apply_re', 'basic_fixed_point'].includes(value.type)
    value = { type: 'compute', data: {
      state: { problem, analyses: { maximization: maximum ?? null } },
      operation: { type: value.type, ...(!unit ? { data } : {}) },
    } }
  }
  return JSON.stringify(value)
}
export function response(value) { return value?.response ?? value }
export function event(value) {
  return value.type === 'response' ? { ...value, data: response(value.data) } : value
}
export function decode(json) { return response(JSON.parse(json)) }
export function decodeEvent(json) { return event(JSON.parse(json)) }
