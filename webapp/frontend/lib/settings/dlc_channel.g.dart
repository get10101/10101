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
      subchannelState: $enumDecodeNullable(_$SubchannelStateEnumMap, json['subchannel_state']),
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
      'subchannel_state': _$SubchannelStateEnumMap[instance.subchannelState],
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

const _$SubchannelStateEnumMap = {
  SubchannelState.established: 'Established',
  SubchannelState.settledOffered: 'SettledOffered',
  SubchannelState.settledReceived: 'SettledReceived',
  SubchannelState.settledAccepted: 'SettledAccepted',
  SubchannelState.settledConfirmed: 'SettledConfirmed',
  SubchannelState.settled: 'Settled',
  SubchannelState.renewOffered: 'RenewOffered',
  SubchannelState.renewAccepted: 'RenewAccepted',
  SubchannelState.renewConfirmed: 'RenewConfirmed',
  SubchannelState.renewFinalized: 'RenewFinalized',
  SubchannelState.closing: 'Closing',
  SubchannelState.collaborativeCloseOffered: 'CollaborativeCloseOffered',
};
