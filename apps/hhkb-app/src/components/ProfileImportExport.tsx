/**
 * Profile import / export modal.
 *
 * Supports:
 *   - Drag-and-drop of `.json` files onto a drop zone
 *   - File picker button
 *   - Exporting the currently-active profile as a pretty-printed `.json`
 *   - Exporting all profiles as a single JSON array
 *
 * Invalid VIA JSON is caught at parse time and surfaced inline so the user
 * can fix the file and re-drop it without losing state.
 */

import { useRef, useState } from 'react';
import {
  Box,
  Button,
  Divider,
  Flex,
  HStack,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Text,
  VStack,
  useToast,
} from '@chakra-ui/react';
import {
  Upload,
  FileJson,
  AlertTriangle,
  Download,
  CheckCircle2,
} from 'lucide-react';
import { useProfileStore } from '../store/profileStore';
import { parseViaProfile, ViaProfile } from '../hhkb/via';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

interface Preview {
  name: string;
  vendorId: string;
  productId: string;
  tags: string[];
  layerCount: number;
  raw: string;
  parsed: ViaProfile;
}

function triggerDownload(filename: string, contents: string): void {
  const blob = new Blob([contents], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function ProfileImportExport({ isOpen, onClose }: Props) {
  const toast = useToast();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const profiles = useProfileStore((s) => s.profiles);
  const activeId = useProfileStore((s) => s.activeProfileId);
  const importProfile = useProfileStore((s) => s.importProfile);
  const exportProfile = useProfileStore((s) => s.exportProfile);
  const exportAll = useProfileStore((s) => s.exportAllProfiles);
  const active = profiles.find((p) => p.id === activeId);

  function loadFile(file: File): void {
    setError(null);
    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result ?? '');
      try {
        const parsed = parseViaProfile(text);
        setPreview({
          name: parsed._roninKB?.profile.name ?? parsed.name,
          vendorId: parsed.vendorId,
          productId: parsed.productId,
          tags: parsed._roninKB?.profile.tags ?? [],
          layerCount: parsed.layers?.length ?? 0,
          raw: text,
          parsed,
        });
      } catch (e) {
        setPreview(null);
        setError(e instanceof Error ? e.message : String(e));
      }
    };
    reader.onerror = () => {
      setError('failed to read file');
    };
    reader.readAsText(file);
  }

  function handleDrop(e: React.DragEvent<HTMLDivElement>): void {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files?.[0];
    if (file) loadFile(file);
  }

  function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>): void {
    const file = e.target.files?.[0];
    if (file) loadFile(file);
    e.target.value = '';
  }

  async function handleConfirmImport(): Promise<void> {
    if (!preview) return;
    try {
      await importProfile(preview.raw);
      toast({
        title: 'Profile imported',
        description: preview.name,
        status: 'success',
        duration: 3000,
      });
      setPreview(null);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  function handleExportCurrent(): void {
    if (!active) {
      toast({
        title: 'No active profile to export',
        status: 'warning',
        duration: 3000,
      });
      return;
    }
    try {
      const json = exportProfile(active.id);
      const safeName = active.name.replace(/[^a-z0-9_-]+/gi, '_');
      triggerDownload(`${safeName || 'profile'}.json`, json);
    } catch (e) {
      toast({
        title: 'Export failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
      });
    }
  }

  function handleExportAll(): void {
    try {
      const json = exportAll();
      triggerDownload('roninKB-profiles.json', json);
    } catch (e) {
      toast({
        title: 'Export failed',
        description: e instanceof Error ? e.message : String(e),
        status: 'error',
      });
    }
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} size="xl">
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>
          <HStack spacing={2}>
            <FileJson size={16} />
            <Text>Profiles</Text>
          </HStack>
        </ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <VStack align="stretch" spacing={5}>
            {/* Import section */}
            <Box>
              <SectionLabel>Import</SectionLabel>
              <Text fontSize="xs" color="text.muted" mb={3}>
                Drop a VIA-compatible JSON file. RoninKB extensions
                (<Text as="span" fontFamily="mono">_roninKB</Text>) are
                preserved losslessly.
              </Text>
              <Box
                borderRadius="lg"
                border="1.5px dashed"
                borderColor={dragOver ? 'accent.primary' : 'border.muted'}
                bg={dragOver ? 'accent.subtle' : 'bg.subtle'}
                p={6}
                textAlign="center"
                transition="background-color 0.15s ease, border-color 0.15s ease"
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOver(true);
                }}
                onDragLeave={() => setDragOver(false)}
                onDrop={handleDrop}
              >
                <Flex direction="column" align="center" gap={2}>
                  <Box color={dragOver ? 'accent.primary' : 'text.muted'}>
                    <Upload size={24} strokeWidth={1.5} />
                  </Box>
                  <Text fontSize="sm" color="text.secondary">
                    Drop a{' '}
                    <Text as="span" fontFamily="mono" color="text.primary">
                      .json
                    </Text>{' '}
                    file here
                  </Text>
                  <Button
                    size="sm"
                    variant="subtle"
                    onClick={() => fileInputRef.current?.click()}
                  >
                    Choose file
                  </Button>
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept=".json,application/json"
                    style={{ display: 'none' }}
                    onChange={handleFileSelect}
                  />
                </Flex>
              </Box>
            </Box>

            {error && (
              <HStack
                spacing={2}
                p={3}
                bg="danger.subtle"
                border="1px solid"
                borderColor="danger"
                borderRadius="md"
                color="danger"
                align="flex-start"
              >
                <Box pt="2px">
                  <AlertTriangle size={14} />
                </Box>
                <Text fontSize="xs" fontFamily="mono">
                  {error}
                </Text>
              </HStack>
            )}

            {preview && (
              <Box
                bg="bg.subtle"
                border="1px solid"
                borderColor="border.subtle"
                p={4}
                borderRadius="lg"
              >
                <HStack mb={3} spacing={2}>
                  <Box color="success">
                    <CheckCircle2 size={14} />
                  </Box>
                  <Text fontSize="xs" fontWeight={500} color="text.primary">
                    Preview
                  </Text>
                </HStack>
                <VStack align="stretch" spacing={1.5}>
                  <MetaRow label="Name" value={preview.name} />
                  <MetaRow
                    label="Vendor / Product"
                    value={`${preview.vendorId} / ${preview.productId}`}
                  />
                  <MetaRow
                    label="Layers"
                    value={String(preview.layerCount)}
                  />
                  {preview.tags.length > 0 && (
                    <MetaRow label="Tags" value={preview.tags.join(', ')} />
                  )}
                </VStack>
              </Box>
            )}

            <Divider />

            {/* Export section */}
            <Box>
              <SectionLabel>Export</SectionLabel>
              <HStack>
                <Button
                  size="sm"
                  leftIcon={<Download size={14} />}
                  onClick={handleExportCurrent}
                  isDisabled={!active}
                >
                  Export current
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  leftIcon={<Download size={14} />}
                  onClick={handleExportAll}
                  isDisabled={profiles.length === 0}
                >
                  Export all ({profiles.length})
                </Button>
              </HStack>
            </Box>
          </VStack>
        </ModalBody>
        <ModalFooter>
          <HStack spacing={2}>
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            <Button
              variant="solid"
              onClick={handleConfirmImport}
              isDisabled={!preview}
            >
              Import profile
            </Button>
          </HStack>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <Text
      fontSize="10px"
      color="text.muted"
      fontFamily="mono"
      textTransform="uppercase"
      letterSpacing="0.08em"
      mb={2}
    >
      {children}
    </Text>
  );
}

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <Flex justify="space-between" fontSize="xs">
      <Text color="text.muted">{label}</Text>
      <Text fontFamily="mono" color="text.primary">
        {value}
      </Text>
    </Flex>
  );
}
