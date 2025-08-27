export async function GET() {
  return new Response('', {
    headers: {
      'Content-Type': 'application/xml'
    }
  });
}
