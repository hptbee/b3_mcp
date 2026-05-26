export function sendWebSocketNotification(socket: { send: (value: string) => void }) {
  socket.send('order notification message');
}

export class NotificationHub {
  SendOrderNotification() {
    return 'SignalR notification sent';
  }
}
