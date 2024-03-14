// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'dlc_channel.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

DlcChannel _$DlcChannelFromJson(Map<String, dynamic> json) => DlcChannel(
      dlcChannelId: json['dlc_channel_id'] as String?,
      contractId: json['contract_id'] as String?,
      channelState: $enumDecode(_$ChannelStateEnumMap, json['channel_state']),
      bufferTxid: json['buffer_txid'] as String?,
      punnishTxid: json['punnish_txid'] as String?,
      fundTxid: json['fund_txid'] as String?,
      fundTxout: json['fund_txout'] as num?,
      feeRate: json['fee_rate'] as num?,
      signedChannelState:
          $enumDecodeNullable(_$SignedChannelStateEnumMap, json['signed_channel_state']),
      closeTxid: json['close_txid'] as String?,
      settleTxid: json['settle_txid'] as String?,
    );

Map<String, dynamic> _$DlcChannelToJson(DlcChannel instance) => <String, dynamic>{
      'dlc_channel_id': instance.dlcChannelId,
      'contract_id': instance.contractId,
      'channel_state': _$ChannelStateEnumMap[instance.channelState]!,
      'buffer_txid': instance.bufferTxid,
      'punnish_txid': instance.punnishTxid,
      'fund_txid': instance.fundTxid,
      'fund_txout': instance.fundTxout,
      'close_txid': instance.closeTxid,
      'settle_txid': instance.settleTxid,
      'fee_rate': instance.feeRate,
      'signed_channel_state': _$SignedChannelStateEnumMap[instance.signedChannelState],
    };

const _$ChannelStateEnumMap = {
  ChannelState.offered: 'Offered',
  ChannelState.accepted: 'Accepted',
  ChannelState.signed: 'Signed',
  ChannelState.closing: 'Closing',
  ChannelState.closed: 'Closed',
  ChannelState.counterClosed: 'CounterClosed',
  ChannelState.closedPunished: 'ClosedPunished',
  ChannelState.collaborativelyClosed: 'CollaborativelyClosed',
  ChannelState.failedAccept: 'FailedAccept',
  ChannelState.failedSign: 'FailedSign',
  ChannelState.cancelled: 'Cancelled',
};

const _$SignedChannelStateEnumMap = {
  SignedChannelState.established: 'Established',
  SignedChannelState.settledOffered: 'SettledOffered',
  SignedChannelState.settledReceived: 'SettledReceived',
  SignedChannelState.settledAccepted: 'SettledAccepted',
  SignedChannelState.settledConfirmed: 'SettledConfirmed',
  SignedChannelState.settled: 'Settled',
  SignedChannelState.renewOffered: 'RenewOffered',
  SignedChannelState.renewAccepted: 'RenewAccepted',
  SignedChannelState.renewConfirmed: 'RenewConfirmed',
  SignedChannelState.renewFinalized: 'RenewFinalized',
  SignedChannelState.closing: 'Closing',
  SignedChannelState.collaborativeCloseOffered: 'CollaborativeCloseOffered',
};
