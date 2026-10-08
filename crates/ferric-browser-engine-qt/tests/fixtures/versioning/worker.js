self.onmessage = event => { self.postMessage(event.data === 'synthetic' ? 'synthetic-ack' : 'unexpected'); };
