import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

class DlcChannel {
  String id;
  ChannelState state;

  DlcChannel({required this.id, required this.state});

  String? getContractId() {
    return null;
  }

  static bridge.DlcChannel apiDummy() {
    return const bridge.DlcChannel(
        dlcChannelId: '', channelState: bridge.ChannelState.failedAccept(), referenceId: '');
  }

  static DlcChannel fromApi(bridge.DlcChannel dlcChannel) {
    switch (dlcChannel.channelState) {
      case (bridge.ChannelState_Signed signed):
        {
          return SignedDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.signed,
              contractId: signed.contractId,
              fundingTxid: signed.fundingTxid,
              closingTxid: signed.closingTxid,
              signedState: SignedChannelState.fromApi(signed.state));
        }
      case (bridge.ChannelState_Offered offered):
        {
          return OfferedDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.offered,
              contractId: offered.contractId);
        }
      case (bridge.ChannelState_Accepted accepted):
        {
          return AcceptedDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.accepted,
              contractId: accepted.contractId);
        }

      case (bridge.ChannelState_Closing closing):
        {
          return ClosingDlcChannel(
            id: dlcChannel.dlcChannelId,
            state: ChannelState.closing,
            contractId: closing.contractId,
            bufferTxid: closing.bufferTxid,
          );
        }
      case (bridge.ChannelState_SettledClosing closing):
        {
          return SettledClosingDlcChannel(
            id: dlcChannel.dlcChannelId,
            state: ChannelState.settledClosing,
            settleTxid: closing.settleTxid,
          );
        }
      case (bridge.ChannelState_Closed c):
        {
          return ClosedDlcChannel(
              id: dlcChannel.dlcChannelId, state: ChannelState.closed, closingTxid: c.closingTxid);
        }
      case (bridge.ChannelState_CounterClosed c):
        {
          return ClosedDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.counterClosed,
              closingTxid: c.closingTxid);
        }
      case (bridge.ChannelState_ClosedPunished _):
        {
          return DlcChannel(id: dlcChannel.dlcChannelId, state: ChannelState.closedPunished);
        }
      case (bridge.ChannelState_CollaborativelyClosed c):
        {
          return ClosedDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.collaborativelyClosed,
              closingTxid: c.closingTxid);
        }
      case (bridge.ChannelState_FailedAccept _):
        {
          return DlcChannel(id: dlcChannel.dlcChannelId, state: ChannelState.failedAccept);
        }
      case (bridge.ChannelState_FailedSign _):
        {
          return DlcChannel(id: dlcChannel.dlcChannelId, state: ChannelState.failedSign);
        }
      case (bridge.ChannelState_Cancelled cancelled):
        {
          return CancelledDlcChannel(
              id: dlcChannel.dlcChannelId,
              state: ChannelState.cancelled,
              contractId: cancelled.contractId);
        }
    }
  }
}

class OfferedDlcChannel extends DlcChannel {
  String contractId;

  OfferedDlcChannel({required super.id, required super.state, required this.contractId});

  @override
  String? getContractId() {
    return contractId;
  }
}

class AcceptedDlcChannel extends DlcChannel {
  String contractId;

  AcceptedDlcChannel({required super.id, required super.state, required this.contractId});

  @override
  String? getContractId() {
    return contractId;
  }
}

class SignedDlcChannel extends DlcChannel {
  String? contractId;
  String fundingTxid;
  String? closingTxid;
  SignedChannelState signedState;

  SignedDlcChannel(
      {required super.id,
      required super.state,
      required this.contractId,
      required this.fundingTxid,
      required this.closingTxid,
      required this.signedState});

  @override
  String? getContractId() {
    return contractId;
  }
}

class ClosingDlcChannel extends DlcChannel {
  String contractId;
  String bufferTxid;

  ClosingDlcChannel(
      {required super.id,
      required super.state,
      required this.contractId,
      required this.bufferTxid});

  @override
  String? getContractId() {
    return contractId;
  }
}

class SettledClosingDlcChannel extends DlcChannel {
  String settleTxid;

  SettledClosingDlcChannel({required super.id, required super.state, required this.settleTxid});
}

class ClosedDlcChannel extends DlcChannel {
  String closingTxid;

  ClosedDlcChannel({required super.id, required super.state, required this.closingTxid});
}

class CancelledDlcChannel extends DlcChannel {
  String contractId;

  CancelledDlcChannel({required super.id, required super.state, required this.contractId});

  @override
  String? getContractId() {
    return contractId;
  }
}

enum ChannelState {
  offered,
  accepted,
  signed,
  closing,
  settledClosing,
  closed,
  counterClosed,
  closedPunished,
  collaborativelyClosed,
  failedAccept,
  failedSign,
  cancelled;

  @override
  String toString() {
    switch (this) {
      case ChannelState.offered:
        return "Offered";
      case ChannelState.accepted:
        return "Accepted";
      case ChannelState.signed:
        return "Signed";
      case ChannelState.closing:
        return "Closing";
      case ChannelState.settledClosing:
        return "Settled Closing";
      case ChannelState.closed:
        return "Closed";
      case ChannelState.counterClosed:
        return "Counter Closed";
      case ChannelState.closedPunished:
        return "Closed Punished";
      case ChannelState.collaborativelyClosed:
        return "Collaboratively Closed";
      case ChannelState.failedAccept:
        return "Failed Accept";
      case ChannelState.failedSign:
        return "Failed Sign";
      case ChannelState.cancelled:
        return "Cancelled";
    }
  }
}

enum SignedChannelState {
  established,
  settledOffered,
  settledReceived,
  settledAccepted,
  settledConfirmed,
  settled,
  renewOffered,
  renewAccepted,
  renewConfirmed,
  renewFinalized,
  closing,
  settledClosing,
  collaborativeCloseOffered;

  static SignedChannelState fromApi(bridge.SignedChannelState state) {
    switch (state) {
      case bridge.SignedChannelState.Established:
        return SignedChannelState.established;
      case bridge.SignedChannelState.SettledOffered:
        return SignedChannelState.settledOffered;
      case bridge.SignedChannelState.SettledReceived:
        return SignedChannelState.settledReceived;
      case bridge.SignedChannelState.SettledAccepted:
        return SignedChannelState.settledAccepted;
      case bridge.SignedChannelState.SettledConfirmed:
        return SignedChannelState.settledConfirmed;
      case bridge.SignedChannelState.Settled:
        return SignedChannelState.settled;
      case bridge.SignedChannelState.RenewOffered:
        return SignedChannelState.renewOffered;
      case bridge.SignedChannelState.RenewAccepted:
        return SignedChannelState.renewAccepted;
      case bridge.SignedChannelState.RenewConfirmed:
        return SignedChannelState.renewConfirmed;
      case bridge.SignedChannelState.RenewFinalized:
        return SignedChannelState.renewFinalized;
      case bridge.SignedChannelState.Closing:
        return SignedChannelState.closing;
      case bridge.SignedChannelState.SettledClosing:
        return SignedChannelState.settledClosing;
      case bridge.SignedChannelState.CollaborativeCloseOffered:
        return SignedChannelState.collaborativeCloseOffered;
    }
  }

  @override
  String toString() {
    switch (this) {
      case SignedChannelState.established:
        return "Established";
      case SignedChannelState.settledOffered:
        return "Settled Offered";
      case SignedChannelState.settledReceived:
        return "Settled Received";
      case SignedChannelState.settledAccepted:
        return "Settled Accepted";
      case SignedChannelState.settledConfirmed:
        return "Settled Confirmed";
      case SignedChannelState.settled:
        return "Settled";
      case SignedChannelState.renewOffered:
        return "Renew Offered";
      case SignedChannelState.renewAccepted:
        return "Renew Accepted";
      case SignedChannelState.renewConfirmed:
        return "Renew Confirmed";
      case SignedChannelState.renewFinalized:
        return "Renew Finalized";
      case SignedChannelState.closing:
        return "Closing";
      case SignedChannelState.settledClosing:
        return "SettledClosing";
      case SignedChannelState.collaborativeCloseOffered:
        return "Collaborative Close Offered";
    }
  }
}
