import { Button } from '@chakra-ui/react';
import { useDeviceStore } from '../store/deviceStore';
import { isWebHidAvailable } from '../hhkb/webhid';

export function ConnectButton() {
  const status = useDeviceStore((s) => s.status);
  const connect = useDeviceStore((s) => s.connect);
  const disconnect = useDeviceStore((s) => s.disconnect);

  if (!isWebHidAvailable()) {
    return (
      <Button size="sm" isDisabled colorScheme="red">
        WebHID not available
      </Button>
    );
  }

  if (status === 'connected') {
    return (
      <Button size="sm" colorScheme="red" variant="outline" onClick={disconnect}>
        Disconnect
      </Button>
    );
  }

  return (
    <Button
      size="sm"
      colorScheme="blue"
      isLoading={status === 'connecting'}
      onClick={connect}
    >
      Connect HHKB
    </Button>
  );
}
