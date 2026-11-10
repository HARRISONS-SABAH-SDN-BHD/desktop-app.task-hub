"""Readable duration labels and numeric timing records."""
def format_duration(seconds):
    ticks=max(0,int(round(float(seconds)*100)))
    parts=[]
    for size,unit in [(8640000,'day'),(360000,'hour'),(6000,'minute')]:
        count,ticks=divmod(ticks,size)
        if count:parts.append(f'{count} {unit}'+('' if count==1 else 's'))
    if ticks or not parts:
        parts.append(f'{ticks/100:.2f} '+('second' if ticks==100 else 'seconds'))
    return ' '.join(parts)

def timing_record(validation,processing=None):
    return {'validation_seconds':round(validation,6),'processing_seconds':None if processing is None else round(processing,6),'total_seconds':round(validation+(processing or 0),6)}

def timing_label(record):
    processing=record.get('processing_seconds')
    return ('Validation: '+format_duration(record['validation_seconds'])+
            '   |   Report generation: '+('Not run' if processing is None else format_duration(processing))+
            '   |   Total: '+format_duration(record['total_seconds']))
