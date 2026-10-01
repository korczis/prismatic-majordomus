// A fixture adapter that answers after a delay: the driver must not order by arrival.
export async function exchange() {
  await new Promise((r) => setTimeout(r, 80));
  return { text: '{"conclusion":"slow answer","stance":"supports"}' };
}
