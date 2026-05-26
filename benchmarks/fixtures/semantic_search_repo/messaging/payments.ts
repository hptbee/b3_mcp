export function publishPaymentCreated() {
  const topic = 'payment.created';
  return { topic, payload: { orderId: 'order-1' } };
}

export function consumeOrderCreated() {
  const topic = 'order.created';
  return { topic, status: 'consumed' };
}
