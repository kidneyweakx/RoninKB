import { Button } from '@chakra-ui/react';
import { PlugZap, Unplug, Ban } from 'lucide-react';
import { useDeviceStore } from '../store/deviceStore';
import { useDaemonStore } from '../store/daemonStore';
import { isWebHidAvailable } from '../hhkb/webhid';

export function ConnectButton() {
  const status = useDeviceStore((s) => s.status);
  const connect = useDeviceStore((s) => s.connect);
  const disconnect = useDeviceStore((s) => s.disconnect);
  const daemonStatus = useDaemonStore((s) => s.status);
  const daemonDeviceConnected = useDaemonStore((s) => s.deviceConnected);

  // When the daemon is online and already holds the keyboard, WebHID
  // availability is irrelevant — we'll route through the daemon instead.
  const daemonWillServeDevice =
    daemonStatus === 'online' && daemonDeviceConnected;

  if (!isWebHidAvailable() && !daemonWillServeDevice) {
    return (
      <Button
        size="sm"
        isDisabled
        variant="subtle"
        leftIcon={<Ban size={14} />}
      >
        WebHID unavailable
      </Button>
    );
  }

  if (status === 'connected') {
    return (
      <Button
        size="sm"
        variant="outline"
        leftIcon={<Unplug size={14} />}
        onClick={disconnect}
      >
        Disconnect
      </Button>
    );
  }

  return (
    <Button
      size="sm"
      variant="solid"
      leftIcon={<PlugZap size={14} />}
      isLoading={status === 'connecting'}
      loadingText="Connecting…"
      onClick={connect}
    >
      {daemonWillServeDevice ? 'Connect via daemon' : 'Connect HHKB'}
    </Button>
  );
}
