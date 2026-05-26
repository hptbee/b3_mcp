import React from 'react';

export function OrderPanel({ orderId }: { orderId: string }) {
  return (
    <section>
      <h2>Order creation status</h2>
      <button>Submit order</button>
      <span>{orderId}</span>
    </section>
  );
}
