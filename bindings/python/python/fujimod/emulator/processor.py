"""Paused CPU inspection for host debuggers and isolated driver callers."""
import json
import weakref
from enum import Enum,auto
from . import registers as regs


class InstructionAction(Enum):
    """Explicit host control after a passive instruction observation."""
    CONTINUE=auto()
    PAUSE=auto()


def _register(identifier):
    if isinstance(identifier, (str, dict)):
        return identifier
    if identifier in regs.GENERAL:
        return {'Arm32': {'R': regs.GENERAL.index(identifier)}}
    return {regs.PC:'Pc', regs.SP:'Sp', regs.LR:{'Arm32':{'R':14}},
            regs.CPSR:'Pstate', regs.SPSR:{'Arm32':'Spsr'},
            regs.CPACR:{'Arm32':'Cpacr'}, regs.FPEXC:{'Arm32':'Fpexc'},
            regs.FPSCR:{'Arm32':'Fpscr'}, regs.FPSID:{'Arm32':'Fpsid'}}[identifier]


class Processor:
    def __init__(self, board, core):
        self.board, self.core = weakref.proxy(board), core

    def reg_read(self, register):
        return self.board.native.reg_read(self.core, json.dumps(_register(register)))

    def reg_write(self, register, value):
        self.board.native.reg_write(self.core, json.dumps(_register(register)), value)

    def cpr_read(self, cp, opc1, crn, crm, opc2, secure, wide):
        return self.board.native.coprocessor(self.core, json.dumps(dict(
            cp=cp, opc1=opc1, crn=crn, crm=crm, opc2=opc2, secure=secure, wide=wide)), None)

    def cpr_write(self, cp, opc1, crn, crm, opc2, secure, wide, value):
        return self.board.native.coprocessor(self.core, json.dumps(dict(
            cp=cp, opc1=opc1, crn=crn, crm=crm, opc2=opc2, secure=secure, wide=wide)), value)

    def mem_read(self, address, size):
        return self.board.native.mem_read(self.core, address, size)

    def mem_write(self, address, data):
        self.board.native.mem_write(self.core, address, bytes(data))


def rprofile_boundary(board):
    """Paused native CPU, IRQ and scheduler observation for R-profile clients."""
    import copy
    native=board.native
    state=json.loads(native.observation())
    registers=tuple(tuple(native.reg_read(core,register) for register in
                    ('"Pc"','{"Arm32":{"R":13}}','"Pstate"'))
                    for core in range(len(state['instructions'])))
    controllers=tuple(state['interrupt_controllers'])
    return (sum(state['instructions']),native.now,copy.deepcopy(board.failure),registers,
            controllers,state['timeline'],state['released'],state['waiting'],
            state['events'],state['next_core'],state['slice_left'])
