export async function POST() {
  return Response.json({ route: 'create order' });
}

export async function GET() {
  return Response.json({ route: 'list orders' });
}
