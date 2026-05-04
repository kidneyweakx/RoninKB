/**
 * First-run setup wizard.
 *
 * Auto-opens on the first launch (after a small delay so the rest of
 * the UI has a chance to render and the daemon probe has kicked off).
 * After completion the flag is persisted in localStorage so it stays
 * closed on subsequent reloads. Re-openable from SettingsPanel.
 */

import { useEffect } from 'react';
import {
  Box,
  Button,
  HStack,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Text,
} from '@chakra-ui/react';
import { Check, Circle } from 'lucide-react';
import { useSetupStore, TOTAL_STEPS } from '../store/setupStore';
import { useBackendStore } from '../store/backendStore';
import { StepWelcome } from './setup/StepWelcome';
import { StepBrowser } from './setup/StepBrowser';
import { StepDaemon } from './setup/StepDaemon';
import { StepBackend } from './setup/StepBackend';
import { StepFirstProfile } from './setup/StepFirstProfile';

export function SetupWizard() {
  const open = useSetupStore((s) => s.open);
  const completed = useSetupStore((s) => s.completed);
  const currentStep = useSetupStore((s) => s.currentStep);
  const goNext = useSetupStore((s) => s.goNext);
  const goBack = useSetupStore((s) => s.goBack);
  const skip = useSetupStore((s) => s.skip);
  const complete = useSetupStore((s) => s.complete);
  const close = useSetupStore((s) => s.close);
  const openManually = useSetupStore((s) => s.openManually);
  const verifiedBackend = useSetupStore((s) => s.verifiedBackend);
  const activeBackend = useBackendStore((s) => s.active);

  // Backend step gates Next on a verified-binding self-attestation. The
  // user can still bail with "Skip setup" — gating just nudges them to
  // confirm the backend works before moving on (M4 §85).
  const nextDisabled =
    currentStep === 3 && (activeBackend === null || verifiedBackend !== activeBackend);

  // Auto-open on first run.
  useEffect(() => {
    if (!completed) {
      const t = setTimeout(() => {
        if (!useSetupStore.getState().completed) {
          openManually();
        }
      }, 500);
      return () => clearTimeout(t);
    }
  }, [completed, openManually]);

  const isFirstRun = !completed;

  return (
    <Modal
      isOpen={open}
      onClose={isFirstRun ? () => undefined : close}
      size="xl"
      closeOnOverlayClick={!isFirstRun}
      closeOnEsc={!isFirstRun}
    >
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>
          <HStack justify="space-between" align="center">
            <Text fontSize="md">Setup RoninKB</Text>
            <StepIndicator current={currentStep} />
          </HStack>
        </ModalHeader>
        <ModalBody>
          {currentStep === 0 && <StepWelcome />}
          {currentStep === 1 && <StepBrowser />}
          {currentStep === 2 && <StepDaemon />}
          {currentStep === 3 && <StepBackend />}
          {currentStep === 4 && <StepFirstProfile onDone={complete} />}
        </ModalBody>
        <ModalFooter>
          <HStack spacing={2} justify="space-between" w="100%">
            <HStack>
              <Button
                variant="ghost"
                size="sm"
                onClick={goBack}
                isDisabled={currentStep === 0}
              >
                Back
              </Button>
              <Button variant="ghost" size="sm" onClick={skip}>
                Skip setup
              </Button>
            </HStack>
            {currentStep < TOTAL_STEPS - 1 ? (
              <Button
                variant="solid"
                size="sm"
                onClick={goNext}
                isDisabled={nextDisabled}
              >
                Next
              </Button>
            ) : null}
          </HStack>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

function StepIndicator({ current }: { current: number }) {
  return (
    <HStack spacing={1}>
      {Array.from({ length: TOTAL_STEPS }, (_, i) => {
        const done = i < current;
        const active = i === current;
        return (
          <Box
            key={i}
            color={
              done
                ? 'success'
                : active
                  ? 'accent.primary'
                  : 'text.muted'
            }
            display="flex"
          >
            {done ? <Check size={12} /> : <Circle size={12} />}
          </Box>
        );
      })}
      <Text
        fontSize="10px"
        fontFamily="mono"
        color="text.muted"
        ml={1}
      >
        {current + 1}/{TOTAL_STEPS}
      </Text>
    </HStack>
  );
}
