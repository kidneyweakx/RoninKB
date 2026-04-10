import { useRef, useState } from 'react';
import {
  Alert,
  AlertIcon,
  Box,
  Button,
  Flex,
  Heading,
  HStack,
  Text,
  useToast,
  VStack,
} from '@chakra-ui/react';
import {
  CheckCircle2,
  FileJson,
  Plus,
  SkipForward,
} from 'lucide-react';
import { useProfileStore } from '../../store/profileStore';
import { parseViaProfile, type ViaProfile } from '../../hhkb/via';

type Choice = 'default' | 'import' | 'skip' | null;

interface Props {
  onDone: () => void;
}

export function StepFirstProfile({ onDone }: Props) {
  const toast = useToast();
  const addProfile = useProfileStore((s) => s.addProfile);
  const setActive = useProfileStore((s) => s.setActiveProfile);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [choice, setChoice] = useState<Choice>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [completed, setCompleted] = useState(false);

  async function createDefault() {
    setBusy(true);
    setError(null);
    try {
      const via: ViaProfile = {
        name: 'Default',
        vendorId: '0x04FE',
        productId: '0x0021',
        matrix: { rows: 8, cols: 8 },
        layers: [],
        _roninKB: {
          version: '1',
          profile: {
            id: crypto.randomUUID(),
            name: 'Default',
            tags: [],
          },
          software: {
            engine: 'kanata',
            engine_version: '',
            config: '',
          },
        },
      };
      await addProfile({
        id: via._roninKB!.profile.id,
        name: 'Default',
        tags: [],
        via,
      });
      await setActive(via._roninKB!.profile.id);
      toast({ title: 'Default profile created', status: 'success' });
      setCompleted(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function pickImport() {
    setChoice('import');
    fileInputRef.current?.click();
  }

  async function handleFile(file: File) {
    setBusy(true);
    setError(null);
    try {
      const text = await file.text();
      const via = parseViaProfile(text);
      if (!via.name || !Array.isArray(via.layers)) {
        throw new Error("not a VIA profile (missing 'name' or 'layers')");
      }
      const id = via._roninKB?.profile.id ?? crypto.randomUUID();
      await addProfile({ id, name: via.name, tags: [], via });
      await setActive(id);
      toast({ title: `Imported '${via.name}'`, status: 'success' });
      setCompleted(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function skip() {
    setChoice('skip');
    setCompleted(true);
  }

  return (
    <VStack align="stretch" spacing={4}>
      <Box>
        <Heading size="md" mb={1}>
          First profile
        </Heading>
        <Text fontSize="sm" color="text.muted">
          Pick how to get started. You can always add more profiles later.
        </Text>
      </Box>

      {!completed && (
        <VStack align="stretch" spacing={2}>
          <ChoiceCard
            icon={<Plus size={18} />}
            title="Create default profile"
            description="A clean slate targeting the HHKB Professional Hybrid."
            onClick={createDefault}
            active={choice === 'default'}
            disabled={busy}
          />
          <ChoiceCard
            icon={<FileJson size={18} />}
            title="Import existing VIA JSON"
            description="Drop a .json file exported from VIA or another RoninKB install."
            onClick={pickImport}
            active={choice === 'import'}
            disabled={busy}
          />
          <ChoiceCard
            icon={<SkipForward size={18} />}
            title="Skip — I'll create one later"
            description="You can add a profile from the Profiles menu anytime."
            onClick={skip}
            active={choice === 'skip'}
            disabled={busy}
          />
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,application/json"
            style={{ display: 'none' }}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void handleFile(f);
              e.target.value = '';
            }}
          />
        </VStack>
      )}

      {error && (
        <Alert status="error" borderRadius="md" fontSize="xs">
          <AlertIcon />
          {error}
        </Alert>
      )}

      {completed && (
        <Box
          border="1px solid"
          borderColor="success"
          bg="success.subtle"
          borderRadius="md"
          p={4}
        >
          <HStack spacing={3}>
            <Box color="success">
              <CheckCircle2 size={24} />
            </Box>
            <Box flex="1">
              <Text fontSize="sm" fontWeight={600}>
                Setup complete
              </Text>
              <Text fontSize="xs" color="text.muted">
                You're all set — happy typing.
              </Text>
            </Box>
            <Button size="sm" variant="solid" onClick={onDone}>
              Finish
            </Button>
          </HStack>
        </Box>
      )}
    </VStack>
  );
}

function ChoiceCard({
  icon,
  title,
  description,
  onClick,
  active,
  disabled,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick: () => void;
  active: boolean;
  disabled: boolean;
}) {
  return (
    <Box
      as="button"
      onClick={onClick}
      disabled={disabled}
      textAlign="left"
      p={4}
      border="1px solid"
      borderColor={active ? 'accent.primary' : 'border.subtle'}
      bg={active ? 'accent.subtle' : 'bg.subtle'}
      borderRadius="md"
      _hover={!disabled ? { borderColor: 'accent.primary' } : undefined}
      opacity={disabled ? 0.6 : 1}
      cursor={disabled ? 'not-allowed' : 'pointer'}
      transition="all 0.15s ease"
      w="100%"
    >
      <Flex gap={3} align="flex-start">
        <Box color={active ? 'accent.primary' : 'text.muted'} mt={0.5}>
          {icon}
        </Box>
        <Box>
          <Text fontSize="sm" fontWeight={600}>
            {title}
          </Text>
          <Text fontSize="xs" color="text.muted">
            {description}
          </Text>
        </Box>
      </Flex>
    </Box>
  );
}
