import 'package:json_annotation/json_annotation.dart';

part 'dlc_channel.g.dart';

enum ChannelState {
  @JsonValue('Offered')
  offered,
  @JsonValue('Accepted')
  accepted,
  @JsonValue('Signed')
  signed,
  @JsonValue('Closing')
  closing,
  @JsonValue('Closed')
  closed,
  @JsonValue('CounterClosed')
  counterClosed,
  @JsonValue('ClosedPunished')
  closedPunished,
  @JsonValue('CollaborativelyClosed')
  collaborativelyClosed,
  @JsonValue('FailedAccept')
  failedAccept,
  @JsonValue('FailedSign')
  failedSign,
  @JsonValue('Cancelled')
  cancelled,
}

enum SubchannelState {
  @JsonValue('Established')
  established,
  @JsonValue('SettledOffered')
  settledOffered,
  @JsonValue('SettledReceived')
  settledReceived,
  @JsonValue('SettledAccepted')
  settledAccepted,
  @JsonValue('SettledConfirmed')
  settledConfirmed,
  @JsonValue('Settled')
  settled,
  @JsonValue('RenewOffered')
  renewOffered,
  @JsonValue('RenewAccepted')
  renewAccepted,
  @JsonValue('RenewConfirmed')
  renewConfirmed,
  @JsonValue('RenewFinalized')
  renewFinalized,
  @JsonValue('Closing')
  closing,
  @JsonValue('CollaborativeCloseOffered')
  collaborativeCloseOffered,
}

@JsonSerializable()
class DlcChannel {
  @JsonKey(name: 'dlc_channel_id')
  final String? dlcChannelId;
  @JsonKey(name: 'contract_id')
  final String? contractId;
  @JsonKey(name: 'channel_state')
  final ChannelState channelState;
  @JsonKey(name: 'buffer_txid')
  final String? bufferTxid;
  @JsonKey(name: 'punnish_txid')
  final String? punnishTxid;
  @JsonKey(name: 'fund_txid')
  final String? fundTxid;
  @JsonKey(name: 'fund_txout')
  final num? fundTxout;
  @JsonKey(name: 'close_txid')
  final String? closeTxid;
  @JsonKey(name: 'settle_txid')
  final String? settleTxid;
  @JsonKey(name: 'fee_rate')
  final num? feeRate;
  @JsonKey(name: 'subchannel_state')
  final SubchannelState? subchannelState;

  DlcChannel({
    this.dlcChannelId,
    this.contractId,
    required this.channelState,
    this.bufferTxid,
    this.punnishTxid,
    this.fundTxid,
    this.fundTxout,
    this.feeRate,
    this.subchannelState,
    this.closeTxid,
    this.settleTxid,
  });

  factory DlcChannel.fromJson(Map<String, dynamic> json) => _$DlcChannelFromJson(json);

  Map<String, dynamic> toJson() => _$DlcChannelToJson(this);
}
