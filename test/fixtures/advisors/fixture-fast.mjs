// A fixture adapter that answers at once.
export async function exchange() {
  return { text: '{"conclusion":"fast answer","stance":"opposes"}' };
}
