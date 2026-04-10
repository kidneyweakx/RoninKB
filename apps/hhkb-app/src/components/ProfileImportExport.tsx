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
      <ModalContent bg="gray.800" color="white">
        <ModalHeader>Import / Export profiles</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <VStack align="stretch" spacing={5}>
            <Box>
              <Text fontSize="sm" color="gray.400" mb={2}>
                Import a VIA-compatible JSON profile. RoninKB extensions
                (<code>_roninKB</code>) are preserved losslessly.
              </Text>
              <Box
                borderRadius="md"
                border="2px dashed"
                borderColor={dragOver ? 'brand.400' : 'gray.600'}
                bg={dragOver ? 'gray.700' : 'gray.900'}
                p={6}
                textAlign="center"
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOver(true);
                }}
                onDragLeave={() => setDragOver(false)}
                onDrop={handleDrop}
              >
                <Text mb={3}>Drop a <code>.json</code> file here</Text>
                <Button
                  size="sm"
                  onClick={() => fileInputRef.current?.click()}
                >
                  Choose file...
                </Button>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept=".json,application/json"
                  style={{ display: 'none' }}
                  onChange={handleFileSelect}
                />
              </Box>
            </Box>

            {error && (
              <Box bg="red.900" color="red.100" p={3} borderRadius="md">
                <Text fontSize="sm">Error: {error}</Text>
              </Box>
            )}

            {preview && (
              <Box bg="gray.900" p={4} borderRadius="md">
                <Text fontWeight="bold" mb={2}>
                  Preview
                </Text>
                <VStack align="stretch" spacing={1} fontSize="sm">
                  <Text>Name: {preview.name}</Text>
                  <Text>
                    Vendor: {preview.vendorId} / Product: {preview.productId}
                  </Text>
                  <Text>Layers: {preview.layerCount}</Text>
                  {preview.tags.length > 0 && (
                    <Text>Tags: {preview.tags.join(', ')}</Text>
                  )}
                </VStack>
              </Box>
            )}

            <Box>
              <Text fontSize="sm" color="gray.400" mb={2}>
                Export
              </Text>
              <HStack>
                <Button
                  size="sm"
                  onClick={handleExportCurrent}
                  isDisabled={!active}
                >
                  Export current
                </Button>
                <Button
                  size="sm"
                  onClick={handleExportAll}
                  isDisabled={profiles.length === 0}
                >
                  Export all
                </Button>
              </HStack>
            </Box>
          </VStack>
        </ModalBody>
        <ModalFooter>
          <HStack>
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            <Button
              colorScheme="brand"
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
